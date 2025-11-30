use std::ops::{Deref, DerefMut};

use crate::{
    clause::{ClauseState, Lit},
    problem::Problem,
};

pub mod clause;
pub mod parser;
pub mod problem;
pub mod utils;

pub struct DPLLSolver<'p> {
    problem: &'p Problem,
    /// The current partial assignment for all variables.
    assignment: PartialAssignment,
    /// History of all variable assignments (decisions and unit propagations).
    /// Used to quickly revert assignments during backtracking.
    var_assignment_history: Vec<VariableAssignment>,
    /// Stores the index within `assign_history` where the assignments for each new decision level begin.
    /// `decision_level_starts[i]` is the index of the first assignment (the decision itself) at decision level $i$.
    /// Required, because after each decision, multiple unit propagations may occur.
    decision_level_starts: Vec<usize>,
    /// The current depth in the search tree (number of branching decisions made).
    /// Level 0 is the initial state before any decisions.
    decision_level: usize,
}

impl<'p> DPLLSolver<'p> {
    pub fn new(problem: &'p Problem) -> Self {
        let num_vars = problem.num_vars;
        DPLLSolver {
            problem,
            assignment: PartialAssignment::with_size(num_vars),
            decision_level_starts: vec![0],
            var_assignment_history: Vec::new(),
            decision_level: 0,
        }
    }

    pub fn solve(&mut self) -> Option<Vec<bool>> {
        let mut next_falsified_lit = self.make_branching_decision();

        'backtrack: loop {
            match self.propagate_units(next_falsified_lit) {
                PropagationResult::Satisfied => {
                    return Some(self.assignment_to_solution());
                }
                PropagationResult::Unsatisfied => {
                    match self.backtrack() {
                        None => {
                            return None; // Cannot backtrack further => UNSAT
                        }
                        Some(falsified_lit) => {
                            next_falsified_lit = falsified_lit;
                        }
                    }
                    continue 'backtrack;
                }
                PropagationResult::Undecided => {
                    // No conflicts & not all clauses satisfied => some clauses are still undecided
                    // Make the next branching decision
                    next_falsified_lit = self.make_branching_decision();
                }
            }
        }
    }

    /// Performs unit propagation starting from the literal that was just falsified.
    fn propagate_units(&mut self, falsified_lit: Lit) -> PropagationResult {
        let mut falsified_lits: Vec<Lit> = vec![falsified_lit];

        // For each literal that was just falsified, check only the affected clauses.
        while let Some(lit) = falsified_lits.pop() {
            'clauses: for clause in self.problem.clauses_containing_lit(lit) {
                match clause.eval_with(&self.assignment) {
                    ClauseState::Satisfied => continue 'clauses, // 1 clause satisfied => check next
                    ClauseState::Unsatisfied => {
                        return PropagationResult::Unsatisfied; // Conflict => backtrack
                    }
                    ClauseState::Undecided(_) => continue 'clauses, // continue checking for conflicts and unit clauses
                    ClauseState::Unit(unit_literal) => {
                        if let Some(val) = self.assignment[unit_literal.var_id()] {
                            // Check if the variable is already assigned the opposite value
                            if val != unit_literal.is_pos() {
                                return PropagationResult::Unsatisfied; // Conflict => backtrack
                            }
                            // Variable already assigned correctly, no action needed
                        } else {
                            // Variable is unassigned. Assign the variable such that the unit literal is true.
                            // => The unit clause will be satisfied.
                            self.assign_variable(unit_literal.var_id(), unit_literal.is_pos());

                            // Unit literal is now true => its negation is false
                            falsified_lits.push(unit_literal.negated());
                        }
                    }
                }
            }
        }
        // All propagations done without conflicts.

        // Check if all clauses are satisfied
        if self
            .problem
            .clauses
            .iter()
            .all(|clause| matches!(clause.eval_with(&self.assignment), ClauseState::Satisfied))
        {
            return PropagationResult::Satisfied;
        }

        // No conflicts & not all clauses satisfied => some clauses are still undecided
        PropagationResult::Undecided
    }

    /// Makes a branching decision by selecting an unassigned variable and assigning it to true.
    fn make_branching_decision(&mut self) -> Lit {
        // Find first unassigned variable
        let decision_var_id = self
            .assignment
            .iter()
            .enumerate()
            .find(|(_, val)| val.is_none())
            .map(|(id, _)| id)
            .expect("BUG: Should always find an unassigned variable when PropagationResult is Undecided.");

        self.decision_level += 1;
        self.decision_level_starts
            .push(self.var_assignment_history.len());

        self.assign_variable(decision_var_id, true);
        // Return the negated literal of the assigned decision variable
        return Lit::new(decision_var_id, true).negated(); // TODO: Return ID instead of Lit
    }

    /// Backtracks to the last level where the decision can be flipped (from true to false).
    /// Returns the **falsified literal** corresponding to the new decision, which initiates the
    /// next round of unit propagation. Returns `None` if the problem is UNSAT.
    fn backtrack(&mut self) -> Option<Lit> {
        'backtrack: loop {
            if self.decision_level == 0 {
                return None; // Cannot backtrack further => UNSAT
            }

            // Revert all variable assignments made by unit propagation (UP) *after* the given decision (D).
            // Do not revert the decision itself, so we try to flip it later.
            self.undo_assignments_after(self.decision_level_starts[self.decision_level]);

            // Now, the branching decision is the last element in the variable assignment history.
            // Attempt to flip the decision.
            // Decision order: true -> false -> backtrack further
            if let Some(prev_decision) = self.var_assignment_history.last_mut() {
                if self.assignment[prev_decision.var_id] == Some(false) {
                    // Decision exhausted: Both true and false have been tried => conflict.
                    // Revert the decision and backtrack further
                    // Example history: [..., D 4=T] -> [...]
                    self.assignment[prev_decision.var_id] = prev_decision.old_value;
                    self.var_assignment_history.pop();
                    self.decision_level_starts.pop();
                    self.decision_level -= 1;
                    continue 'backtrack;
                } else {
                    // Flip decision: Currently true -> try to assign the variable to false.
                    // We reuse the assignment history entry, so no need to modify it.
                    self.assignment[prev_decision.var_id] = Some(false);
                    self.decision_level_starts.pop(); // We exhausted all decision options at this level.
                    self.decision_level -= 1;

                    // The variable 'V' is now false => The literal 'V' is falsified and initiates the next round of unit propagation.
                    return Some(Lit::new(prev_decision.var_id, true));
                }
            } else {
                panic!("BUG: Decision history index mismatch in backtrack."); // decision_level > 0 => should not happen
            }
        }
    }

    /// Undoes all variable assignments made after the given index in the assignment history.
    /// The assignment at and before the boundary index is retained.
    /// Example history: [..., D 4=T, UP 5=F, UP 6=T] with boundary_idx pointing to 'D 4=T'
    ///               => [..., D 4=T]
    fn undo_assignments_after(&mut self, boundary_idx: usize) {
        while self.var_assignment_history.len() > boundary_idx + 1 {
            if let Some(to_revert) = self.var_assignment_history.pop() {
                self.assignment[to_revert.var_id] = to_revert.old_value; // Restore previous assignment
            }
        }
    }

    /// Assigns a variable and records the assignment for backtracking.
    fn assign_variable(&mut self, var_id: usize, value: bool) {
        self.var_assignment_history.push(VariableAssignment {
            var_id,
            old_value: self.assignment[var_id],
        });
        self.assignment[var_id] = Some(value);
    }

    fn assignment_to_solution(&self) -> Vec<bool> {
        self.assignment
            .iter()
            .map(|&val| val.unwrap_or(false))
            .collect()
    }
}

// ---------------------
// --- Utility types ---
// ---------------------

enum PropagationResult {
    Satisfied,
    Unsatisfied,
    Undecided,
}

#[derive(Debug)]
struct VariableAssignment {
    /// The ID of the variable that was assigned a value.
    var_id: usize,
    /// The previous value of the variable before assignment.
    old_value: Option<bool>,
}

/// Represents a partial assignment of boolean variables.
/// Each variable can be assigned `Some(true)`, `Some(false)`, or `None` (unassigned).
#[derive(Debug, Clone)]
pub struct PartialAssignment(Vec<Option<bool>>);

impl PartialAssignment {
    fn with_size(size: usize) -> Self {
        PartialAssignment(vec![None; size])
    }
}

impl Deref for PartialAssignment {
    type Target = Vec<Option<bool>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PartialAssignment {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
