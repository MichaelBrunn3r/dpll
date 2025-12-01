use crate::{clause::Lit, clause::VariableId};
use std::ops::{Deref, Index};

/// Manages the partial assignment of variables during the DPLL solving process.
/// Supports decisions, unit propagations, and decision backtracking.
pub struct PartialAssignment<'a> {
    /// The current partial assignment for all variables.
    /// None=unassigned, Some(bool)=assigned to true/false.
    current_state: &'a mut [VarState],

    /// A chronological stack of all variable assignments (decisions & unit propagations).
    /// Used to undo assignments during backtracking.
    history: Vec<VariableId>,

    /// Indices into the history that mark the start of each decision level.
    /// `decision_marks[i]` points to the index in `history` where the decision variable for level `i+1` is stored.
    decision_marks: Vec<usize>,
}

impl<'a> PartialAssignment<'a> {
    /// Creates a new Assignment state with the given initial assignment.
    /// The initial assignment will be treated as level 0 (no decisions made yet).
    pub fn with_assignment(initial_assignment: &'a mut [VarState]) -> Self {
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
    #[inline(always)]
    pub fn is_assigned(&self, var: VariableId) -> bool {
        self.current_state[var].is_assigned()
    }

    /// Assign a variable during Unit Propagation.
    /// Assumes the variable is unassigned.
    pub fn propagate(&mut self, var: VariableId, val: bool) {
        debug_assert!(
            self.current_state[var].is_unassigned(),
            "Trying to propagate the already assigned variable {}.",
            var
        );
        self.current_state[var] = VarState::new_assigned(val);
        self.history.push(var);
    }

    /// Starts a new decision level by assigning a chosen variable to `true`.
    pub fn decide(&mut self, var: VariableId) {
        debug_assert!(self.current_state[var].is_unassigned());

        // Mark the start of this new decision level.
        self.decision_marks.push(self.history.len());

        // Always try true first. If this leads to a conflict, we will backtrack and try false.
        self.current_state[var] = VarState::new_assigned(true);
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

            if self.current_state[decision_var].is_true() {
                // We tried true without success => try false next.
                self.current_state[decision_var] = VarState::new_assigned(false);
                return Some(Lit::new(decision_var, true));
            } else {
                // We tried both true and false with no success
                // => All options at this level are exhausted. Try the next higher level.
                self.current_state[decision_var] = VarState::new_unassigned();
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
            self.current_state[var] = VarState::new_unassigned();
        }
        *level_start
    }

    /// Converts the partial assignment to a full solution.
    /// Unassigned variables default to `false`.
    pub fn to_solution(&self) -> Vec<bool> {
        Self::assignment_to_solution(&self.current_state)
    }

    pub fn assignment_to_solution(assignment: &[VarState]) -> Vec<bool> {
        assignment
            .iter()
            .map(|&var| var.get_value_or(false)) // Default unassigned variables to false
            .collect()
    }

    /// Gets the VarState for the given variable without bounds checking.
    /// # Safety
    /// The caller must ensure that `var` is a valid index into the current assignment.
    #[inline(always)]
    pub fn get_unchecked(&self, var: VariableId) -> VarState {
        Self::get_unchecked_from(&self.current_state, var)
    }

    /// Gets the VarState for the given variable from the provided assignment slice without bounds checking.
    /// # Safety
    /// The caller must ensure that `var` is a valid index into `assignment`.
    #[inline(always)]
    pub fn get_unchecked_from(assignment: &[VarState], var: VariableId) -> VarState {
        debug_assert!(
            var < assignment.len(),
            "Variable {} out of bounds for assignment of length {}.",
            var,
            assignment.len()
        );
        unsafe { *assignment.get_unchecked(var) }
    }
}

impl<'a> Index<usize> for PartialAssignment<'a> {
    type Output = VarState;

    fn index(&self, index: usize) -> &Self::Output {
        &self.current_state[index]
    }
}

impl<'a> Deref for PartialAssignment<'a> {
    type Target = [VarState];

    fn deref(&self) -> &Self::Target {
        &self.current_state
    }
}

/// Represents the state of a variable in a partial assignment.
/// Can be unassigned, assigned to true, or assigned to false.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)] // Ensure VarState has the same memory layout as u8
pub struct VarState(u8);

impl VarState {
    const FALSE: u8 = 0;
    const TRUE: u8 = 1;
    const UNASSIGNED: u8 = 0xFF;

    #[inline(always)]
    pub fn new_unassigned() -> Self {
        Self(Self::UNASSIGNED)
    }

    #[inline(always)]
    pub fn new_assigned(val: bool) -> Self {
        Self(if val { Self::TRUE } else { Self::FALSE })
    }

    #[inline(always)]
    pub fn is_assigned(self) -> bool {
        self.0 != Self::UNASSIGNED
    }

    #[inline(always)]
    pub fn is_unassigned(self) -> bool {
        self.0 == Self::UNASSIGNED
    }

    /// Returns the assigned boolean value, or the given default if unassigned.
    #[inline(always)]
    pub fn get_value_or(self, default: bool) -> bool {
        match self.0 {
            Self::TRUE => true,
            Self::FALSE => false,
            Self::UNASSIGNED => default,
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub fn is_true(self) -> bool {
        self.0 == Self::TRUE
    }

    /// Checks if the variable is assigned to the given boolean value.
    /// Returns false if unassigned or assigned to the opposite value.
    #[inline(always)]
    pub fn is_bool(self, val: bool) -> bool {
        self.0 == (val as u8)
    }
}

impl Default for VarState {
    fn default() -> Self {
        Self::new_unassigned()
    }
}
