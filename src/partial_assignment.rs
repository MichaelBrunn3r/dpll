use crate::{clause::Lit, clause::VariableId};
use std::ops::Index;

/// Manages the partial assignment of variables during the DPLL solving process.
/// Supports decisions, unit propagations, and decision backtracking.
pub struct PartialAssignment {
    /// The current partial assignment for all variables.
    /// None=unassigned, Some(bool)=assigned to true/false.
    current_state: Vec<Option<bool>>,

    /// A chronological stack of all variable assignments (decisions & unit propagations).
    /// Used to undo assignments during backtracking.
    history: Vec<VariableId>,

    /// Indices into the history that mark the start of each decision level.
    /// `decision_marks[i]` points to the index in `history` where the decision variable for level `i+1` is stored.
    decision_marks: Vec<usize>,
}

impl PartialAssignment {
    /// Creates a new Assignment state with no variables assigned.
    pub fn new(num_vars: usize) -> Self {
        PartialAssignment {
            current_state: vec![None; num_vars],
            history: Vec::with_capacity(num_vars),
            decision_marks: Vec::new(), // Level 0 is implicit
        }
    }

    /// Creates a new Assignment state with the given initial assignment.
    /// The initial assignment will be treated as level 0 (no decisions made yet).
    pub fn with_assignment(initial_assignment: Vec<Option<bool>>) -> Self {
        PartialAssignment {
            current_state: initial_assignment,
            history: Vec::new(),
            decision_marks: Vec::new(),
        }
    }

    /// Returns the current decision level (depth of the search tree).
    pub fn decision_level(&self) -> usize {
        self.decision_marks.len()
    }

    /// Check if a variable is assigned.
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
        self.current_state[var] = Some(val);
        self.history.push(var);
    }

    /// Starts a new decision level by assigning a chosen variable to `true`.
    pub fn decide(&mut self, var: VariableId) {
        debug_assert!(self.current_state[var].is_none());

        // Mark the start of this new decision level.
        self.decision_marks.push(self.history.len());

        // Always try true first. If this leads to a conflict, we will backtrack and try false.
        self.current_state[var] = Some(true);
        self.history.push(var);
    }

    /// Backtracks to the highest decision level that hasn't been fully explored.
    ///
    /// 1. Reverts unit propagations at the current level.
    /// 2. Checks the previously explored decision:
    ///   - Tried 'true': Try false next. Returns the literal that was falsified because of this change.
    ///   - Tried 'false': All options explored. Try exploring the next higher decision level.
    ///
    /// Returns `None` if no further backtracking is possible (all options exhausted).
    pub fn backtrack(&mut self) -> Option<Lit> {
        loop {
            // Check if we can backtrack further.
            if self.decision_marks.is_empty() {
                return None;
            }

            // Undo all propagations that happened *after* the decision for this level.
            let decision_idx = self.undo_current_unit_propagations();

            let decision_var = self.history[decision_idx];
            let decision_value = self.current_state[decision_var].unwrap();

            if decision_value == true {
                // We tried true without success => try false next.
                self.current_state[decision_var] = Some(false);
                return Some(Lit::new(decision_var, true));
            } else {
                // We tried both true and false with no success
                // => All options at this level are exhausted. Try the next higher level.
                self.current_state[decision_var] = None;
                self.history.pop();
                self.decision_marks.pop();
                continue;
            };
        }
    }

    /// Undoes unit propagations for the current level, leaving the decision variable intact.
    /// Returns the index of the decision variable in the history.
    fn undo_current_unit_propagations(&mut self) -> usize {
        let level_start = self.decision_marks.last().unwrap();

        // Pop everything after the decision variable
        while self.history.len() > level_start + 1 {
            let var = self.history.pop().unwrap();
            self.current_state[var] = None;
        }
        *level_start
    }

    /// Converts the partial assignment to a full solution.
    /// Unassigned variables default to `false`.
    pub fn to_solution(&self) -> Vec<bool> {
        self.current_state
            .iter()
            .map(|&val| val.unwrap_or(false)) // Default unassigned variables to false
            .collect()
    }
}

impl Index<usize> for PartialAssignment {
    type Output = Option<bool>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.current_state[index]
    }
}
