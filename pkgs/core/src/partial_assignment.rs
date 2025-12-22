use itertools::Itertools;

use crate::{
    decision_path::DecisionPath,
    lit::{Lit, VariableID},
    opt_bool::OptBool,
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
    history: Vec<VariableID>,

    /// Indices into the history that mark the start of each decision level.
    /// `decision_marks[i]` points to the index in `history` where the decision variable for level `i+1` is stored.
    decision_marks: Vec<usize>,
    /// The number of currently assigned variables.
    num_assigned: usize,
    initial_decision_level: usize,
}

impl PartialAssignment {
    pub fn with_decisions(num_vars: usize, initial_decisions: &DecisionPath) -> Self {
        let num_assigned = initial_decisions.0.len();
        let initial_decision_level = num_assigned;

        let history = initial_decisions
            .0
            .iter()
            .map(|lit| lit.var())
            .collect_vec();

        debug_assert!(
            history.len() == initial_decision_level,
            "Initial assignment has {} assigned variables, but initial decision level is {}.",
            history.len(),
            initial_decision_level
        );

        PartialAssignment {
            current_state: initial_decisions.to_assignment(num_vars),
            decision_marks: (0..initial_decision_level).collect_vec(),
            history,
            num_assigned,
            initial_decision_level,
        }
    }

    /// Returns the current decision level (depth of the search tree).
    pub fn decision_level(&self) -> usize {
        self.decision_marks.len()
    }

    /// Check if a variable is assigned.
    #[inline(always)]
    pub fn is_assigned(&self, var: VariableID) -> bool {
        self.current_state[var].is_some()
    }

    /// Assign a variable during Unit Propagation.
    /// Assumes the variable is unassigned.
    pub fn propagate(&mut self, var: VariableID, val: bool) {
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
    pub fn decide(&mut self, var: VariableID) {
        debug_assert!(self.current_state[var].is_none());

        // Mark the start of this new decision level.
        self.decision_marks.push(self.history.len());

        // Always try true first. If this leads to a conflict, we will backtrack and try false.
        self.current_state[var] = OptBool::True;
        self.num_assigned += 1;
        self.history.push(var);
    }

    /// Backtracks to the highest decision level that hasn't been fully explored.
    pub fn backtrack<F>(&mut self, mut on_unassign_var: F) -> Option<Lit>
    where
        F: FnMut(VariableID),
    {
        loop {
            use BacktrackResult::*;
            match self.backtrack_once(&mut on_unassign_var) {
                TryAlternative(falsified_lit) => return Some(falsified_lit),
                NoMoreDecisions => {
                    return None;
                }
                ContinueBacktracking => {
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
    pub fn backtrack_once<F>(&mut self, mut on_unassign_var: F) -> BacktrackResult
    where
        F: FnMut(VariableID),
    {
        // Check if we can backtrack further.
        if self.decision_marks.len() <= self.initial_decision_level {
            return BacktrackResult::NoMoreDecisions;
        }

        // Undo all propagations that happened *after* the decision for this level.
        let decision_idx = self.undo_current_unit_propagations(&mut on_unassign_var);
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

            // Notify that the variable is now unassigned.
            on_unassign_var(decision_var);

            self.history.pop();
            self.decision_marks.pop();
            return BacktrackResult::ContinueBacktracking;
        };
    }

    /// Undoes unit propagations for the current level, leaving the decision variable intact.
    /// Returns the index of the decision variable in the history.
    fn undo_current_unit_propagations<F>(&mut self, mut on_unassign_var: F) -> usize
    where
        F: FnMut(VariableID),
    {
        let level_start = self.decision_marks.last().unwrap();

        // Pop everything after the decision variable
        while self.history.len() > level_start + 1 {
            let var = self.history.pop().unwrap();
            self.current_state[var] = OptBool::Unassigned;
            self.num_assigned -= 1;

            // Notify that the variable is now unassigned.
            on_unassign_var(var);
        }
        *level_start
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

    /// Returns the literal of the last decision made, or None if no decisions have been made.
    pub fn last_decision_lit(&self) -> Option<Lit> {
        if let Some(&mark) = self.decision_marks.last() {
            let var = self.history[mark];
            let val = self.current_state[var].unwrap();
            Some(Lit::new(var, val))
        } else {
            None
        }
    }

    /// Gets the VarState for the given variable without bounds checking.
    /// # Safety
    /// The caller must ensure that `var` is a valid index into the current assignment.
    #[inline(always)]
    pub fn get_unchecked(&self, var: VariableID) -> OptBool {
        Self::get_unchecked_from(&self.current_state, var)
    }

    /// Gets the OptBool for the given variable from the provided assignment slice without bounds checking.
    /// # Safety
    /// The caller must ensure that `var` is a valid index into `assignment`.
    #[inline(always)]
    pub fn get_unchecked_from(assignment: &[OptBool], var: VariableID) -> OptBool {
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

    // -------------
    // --- Utils ---
    // -------------

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

    /// Extracts the sequence of variable assignment decisions up to the given decision level into the provided buffer.
    pub fn extract_decisions_upto(&self, level: usize, buffer: &mut Vec<Lit>) {
        for &decision_idx in self.decision_marks.iter().take(level) {
            let var = self.history[decision_idx];
            // Safety: We know var is assigned if it is in history
            let val = self.current_state[var].unwrap();
            buffer.push(Lit::new(var, val));
        }

        debug_assert!(
            buffer.len() == level,
            "Extracted {}, but we wanted up to level {}. Initial level: {}",
            buffer.len(),
            level,
            self.initial_decision_level
        );
    }

    /// Extracts the current sequence of variable assignment decisions into the provided buffer.
    pub fn extract_decisions(&self, buffer: &mut Vec<Lit>) {
        self.extract_decisions_upto(self.decision_level(), buffer);
    }

    /// Extracts decisions starting from `start_level` until (and including) the next decision assigned to `true`.
    pub fn extract_decisions_until_next_true(&self, start_level: usize, buffer: &mut Vec<Lit>) {
        for (level, &decision_idx) in self.decision_marks.iter().enumerate() {
            let var = self.history[decision_idx];
            // Safety: Variables in history are always assigned.
            let val = self.current_state[var].unwrap();

            buffer.push(Lit::new(var, val));

            // Only check the stop condition if we have reached the start_level.
            if level >= start_level && val {
                return;
            }
        }
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
    ContinueBacktracking,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_decisions() {
        let initial_decisions = DecisionPath(vec![
            Lit::new(1, false),
            Lit::new(3, true),
            Lit::new(2, true),
            Lit::new(0, true),
            Lit::new(4, false),
        ]);
        let assignment = PartialAssignment::with_decisions(8, &initial_decisions);

        assert_eq!(assignment.decision_level(), 5);
        assert_eq!(assignment.last_decision(), OptBool::from(false));

        assert_eq!(assignment[0], OptBool::from(true));
        assert_eq!(assignment[1], OptBool::from(false));
        assert_eq!(assignment[2], OptBool::from(true));
        assert_eq!(assignment[3], OptBool::from(true));
        assert_eq!(assignment[4], OptBool::from(false));
        assert_eq!(assignment[5], OptBool::Unassigned);
        assert_eq!(assignment[6], OptBool::Unassigned);
        assert_eq!(assignment[7], OptBool::Unassigned);
    }

    #[test]
    fn test_extract_decisions() {
        let initial_decisions = DecisionPath(vec![
            Lit::new(1, false),
            Lit::new(3, true),
            Lit::new(2, true),
            Lit::new(0, true),
            Lit::new(4, false),
        ]);
        let assignment = PartialAssignment::with_decisions(8, &initial_decisions);

        let mut buffer = Vec::new();
        assignment.extract_decisions(&mut buffer);

        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer[0], Lit::new(1, false));
        assert_eq!(buffer[1], Lit::new(3, true));
        assert_eq!(buffer[2], Lit::new(2, true));
        assert_eq!(buffer[3], Lit::new(0, true));
        assert_eq!(buffer[4], Lit::new(4, false));
    }

    #[test]
    fn test_extract_decisions_until_next_true() {
        let initial_decisions = DecisionPath(vec![
            Lit::new(1, false),
            Lit::new(3, true),
            Lit::new(2, false), // 3rd decision
            Lit::new(0, false),
            Lit::new(4, false),
            Lit::new(6, true), // next true decision
            Lit::new(5, false),
        ]);

        let assignment = PartialAssignment::with_decisions(10, &initial_decisions);

        let mut buffer = Vec::new();
        assignment.extract_decisions_until_next_true(2, &mut buffer);

        assert_eq!(buffer.len(), 6);
        assert_eq!(buffer[0], Lit::new(1, false));
        assert_eq!(buffer[1], Lit::new(3, true));
        assert_eq!(buffer[2], Lit::new(2, false));
        assert_eq!(buffer[3], Lit::new(0, false));
        assert_eq!(buffer[4], Lit::new(4, false));
        assert_eq!(buffer[5], Lit::new(6, true));
    }
}
