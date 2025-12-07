use itertools::Itertools;

use crate::{
    clause::{Lit, VariableId},
    utils::opt_bool::OptBool,
};
use std::ops::{Deref, Index};

/// Manages the partial assignment of variables during the DPLL solving process.
/// Supports decisions, unit propagations, and decision backtracking.
pub struct PartialAssignment {
    /// The current partial assignment for all variables.
    /// None=unassigned, Some(bool)=assigned to true/false.
    current_state: Vec<OptBool>,

    /// A chronological stack of all variable assignments (decisions & unit propagations).
    /// Used to undo assignments during backtracking.
    history: Vec<VariableId>,

    /// Indices into the history that mark the start of each decision level.
    /// `decision_marks[i]` points to the index in `history` where the decision variable for level `i+1` is stored.
    decision_marks: Vec<usize>,
    /// The number of currently assigned variables.
    num_assigned: usize,
    initial_decision_level: usize,
}

impl PartialAssignment {
    /// Creates a new Assignment state with the given initial assignment.
    /// The initial assignment will be treated as level 0 (no decisions made yet).
    pub fn with_assignment(
        initial_assignment: Vec<OptBool>,
        initial_decision_level: usize,
    ) -> Self {
        let num_assigned = initial_assignment
            .iter()
            .filter(|&state| state.is_some())
            .count();

        debug_assert!(
            num_assigned == initial_decision_level,
            "Initial assignment has {} assigned variables, but initial decision level is {}.",
            num_assigned,
            initial_decision_level
        );

        let history = initial_assignment
            .iter()
            .enumerate()
            .filter_map(|(var, &state)| if state.is_some() { Some(var) } else { None })
            .collect_vec();

        debug_assert!(
            history.len() == initial_decision_level,
            "Initial assignment has {} assigned variables, but initial decision level is {}.",
            history.len(),
            initial_decision_level
        );

        PartialAssignment {
            history,
            decision_marks: Vec::new(),
            num_assigned,
            current_state: initial_assignment,
            initial_decision_level,
        }
    }

    /// Returns the current decision level (depth of the search tree).
    pub fn decision_level(&self) -> usize {
        self.decision_marks.len() + self.initial_decision_level
    }

    /// Check if a variable is assigned.
    #[inline(always)]
    pub fn is_assigned(&self, var: VariableId) -> bool {
        self.current_state[var].is_some()
    }

    /// Assign a variable during Unit Propagation.
    /// Assumes the variable is unassigned.
    pub fn propagate(&mut self, var: VariableId, val: bool) {
        debug_assert!(
            self.current_state[var].is_none(),
            "Trying to propagate the already assigned variable {}.",
            var
        );
        self.current_state[var] = OptBool::from(val);
        self.num_assigned += 1;
        self.history.push(var);
    }

    /// Starts a new decision level by assigning a chosen variable to `true`.
    pub fn decide(&mut self, var: VariableId) {
        debug_assert!(self.current_state[var].is_none());

        // Mark the start of this new decision level.
        self.decision_marks.push(self.history.len());

        // Always try true first. If this leads to a conflict, we will backtrack and try false.
        self.current_state[var] = OptBool::True;
        self.num_assigned += 1;
        self.history.push(var);
    }

    /// Backtracks to the highest decision level that hasn't been fully explored.
    pub fn backtrack(&mut self) -> Option<Lit> {
        loop {
            match self.backtrack_once() {
                BacktrackResult::TryAlternative(falsified_lit) => return Some(falsified_lit),
                BacktrackResult::NoMoreDecisions => {
                    return None;
                }
                BacktrackResult::Continue => {
                    continue;
                }
            }
        }
    }

    /// Attempts to backtrack one decision level.
    ///
    /// 1. Reverts unit propagations at the current level.
    /// 2. Checks the previously explored decision:
    ///   - Tried 'true': Try false next. Returns the literal that was falsified because of this change.
    ///   - Tried 'false': All options explored. Returns None to indicate the need to backtrack further.
    ///
    /// Returns `None` if the current level has been fully explored and backtracking should continue to the next higher level.
    pub fn backtrack_once(&mut self) -> BacktrackResult {
        // Check if we can backtrack further.
        if self.decision_marks.is_empty() {
            return BacktrackResult::NoMoreDecisions;
        }

        // Undo all propagations that happened *after* the decision for this level.
        let decision_idx = self.undo_current_unit_propagations();
        let decision_var = self.history[decision_idx];

        if self.current_state[decision_var].is_true() {
            // We tried true without success => try false next.
            self.current_state[decision_var] = OptBool::False;
            return BacktrackResult::TryAlternative(Lit::new(decision_var, true));
        } else {
            // We tried both true and false with no success
            // => All options at this level are exhausted. Try the next higher level.
            self.current_state[decision_var] = OptBool::Unassigned;
            self.num_assigned -= 1;
            self.history.pop();
            self.decision_marks.pop();
            return BacktrackResult::Continue;
        };
    }

    /// Undoes unit propagations for the current level, leaving the decision variable intact.
    /// Returns the index of the decision variable in the history.
    fn undo_current_unit_propagations(&mut self) -> usize {
        let level_start = self.decision_marks.last().unwrap();

        // Pop everything after the decision variable
        while self.history.len() > level_start + 1 {
            let var = self.history.pop().unwrap();
            self.current_state[var] = OptBool::Unassigned;
            self.num_assigned -= 1;
        }
        *level_start
    }

    /// Extracts the current sequence of variable assignment decisions into the provided buffer.
    pub fn extract_decision(&self) -> Vec<(VariableId, bool)> {
        let mut buffer = Vec::new();

        // first, extract the initial decisions
        for i in 0..self.initial_decision_level {
            let var = self.history[i];
            let val = self.current_state[var].unwrap();
            buffer.push((var, val));
        }

        for &decision_idx in &self.decision_marks {
            let var = self.history[decision_idx];
            let val = self.current_state[var].unwrap();
            buffer.push((var, val));
        }

        debug_assert!(
            buffer.len() == self.decision_level(),
            "Extracted {}, but we are @{}. Initial level: {}",
            buffer.len(),
            self.decision_level(),
            self.initial_decision_level
        );

        buffer
    }

    /// Returns the value of the last decision made, or None if no decisions have been made.
    pub fn last_decision(&self) -> OptBool {
        if let Some(&decision_idx) = self.decision_marks.last() {
            let var = self.history[decision_idx];
            self.current_state[var]
        } else {
            OptBool::Unassigned
        }
    }

    /// Converts the partial assignment to a full solution.
    /// Unassigned variables default to `false`.
    pub fn to_solution(&self) -> Vec<bool> {
        Self::assignment_to_solution(&self.current_state)
    }

    pub fn assignment_to_solution(assignment: &[OptBool]) -> Vec<bool> {
        assignment
            .iter()
            .map(|&var| var.unwrap_or(false)) // Default unassigned variables to false
            .collect()
    }

    /// Gets the VarState for the given variable without bounds checking.
    /// # Safety
    /// The caller must ensure that `var` is a valid index into the current assignment.
    #[inline(always)]
    pub fn get_unchecked(&self, var: VariableId) -> OptBool {
        Self::get_unchecked_from(&self.current_state, var)
    }

    /// Gets the OptBool for the given variable from the provided assignment slice without bounds checking.
    /// # Safety
    /// The caller must ensure that `var` is a valid index into `assignment`.
    #[inline(always)]
    pub fn get_unchecked_from(assignment: &[OptBool], var: VariableId) -> OptBool {
        debug_assert!(
            var < assignment.len(),
            "Variable {} out of bounds for assignment of length {}.",
            var,
            assignment.len()
        );
        unsafe { *assignment.get_unchecked(var) }
    }

    /// Checks if all variables are assigned.
    pub fn is_complete(&self) -> bool {
        self.num_assigned == self.current_state.len()
    }
}

impl Index<usize> for PartialAssignment {
    type Output = OptBool;

    fn index(&self, index: usize) -> &Self::Output {
        &self.current_state[index]
    }
}

impl Deref for PartialAssignment {
    type Target = [OptBool];

    fn deref(&self) -> &Self::Target {
        &self.current_state
    }
}

pub enum BacktrackResult {
    TryAlternative(Lit),
    NoMoreDecisions,
    Continue,
}
