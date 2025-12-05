use stackvector::StackVec;
use std::{iter::Cloned, slice::Iter};

use crate::{
    clause::{ClauseState, Lit},
    constants::MAX_FALSIFIED_LITS,
    partial_assignment::{PartialAssignment, VarState},
    problem::Problem,
    utils::NonExhaustingCursor,
};

pub struct DPLLSolver<'a> {
    problem: &'a Problem,
    pub assignment: PartialAssignment<'a>,
    /// Reusable buffer for literals that have just been falsified during unit propagation.
    falsified_lits_buffer: StackVec<[Lit; MAX_FALSIFIED_LITS]>,
    /// Cursor to keep track of which variable to consider next for branching decisions.
    decision_candidate_cursor: NonExhaustingCursor,
}

impl<'a> DPLLSolver<'a> {
    pub fn with_assignment(problem: &'a Problem, initial_assignment: &'a mut [VarState]) -> Self {
        debug_assert!(
            initial_assignment.len() == problem.num_vars,
            "Initial assignment length must match number of variables."
        );

        DPLLSolver {
            problem,
            assignment: PartialAssignment::with_assignment(initial_assignment),
            falsified_lits_buffer: StackVec::new(),
            decision_candidate_cursor: NonExhaustingCursor::new(),
        }
    }

    pub fn solve(&mut self) -> Option<Vec<bool>> {
        let mut falsified_lit = self.make_branching_decision();
        loop {
            match self.step(falsified_lit) {
                SolverAction::SAT => {
                    return Some(self.assignment.to_solution());
                }
                SolverAction::UNSAT => {
                    return None;
                }
                SolverAction::Continue(next_falsified_lit) => {
                    falsified_lit = next_falsified_lit;
                }
            }
        }
    }

    pub fn step(&mut self, next_falsified_lit: Lit) -> SolverAction {
        match self.propagate_units(next_falsified_lit) {
            PropagationResult::SAT => {
                return SolverAction::SAT;
            }
            PropagationResult::UNSAT => {
                self.decision_candidate_cursor.reset();
                match self.assignment.backtrack() {
                    None => {
                        // Cannot backtrack further => UNSAT
                        return SolverAction::UNSAT;
                    }
                    Some(falsified_lit) => {
                        return SolverAction::Continue(falsified_lit);
                    }
                }
            }
            PropagationResult::Undecided => {
                // No conflicts & not all clauses satisfied => some clauses are still undecided
                // Make the next branching decision
                return SolverAction::Continue(self.make_branching_decision());
            }
        }
    }

    /// Performs unit propagation starting from the literal that was just falsified.
    fn propagate_units(&mut self, falsified_lit: Lit) -> PropagationResult {
        self.falsified_lits_buffer.clear();
        self.falsified_lits_buffer.push(falsified_lit);

        // For each literal that was just falsified, check only the affected clauses.
        while let Some(lit) = self.falsified_lits_buffer.pop() {
            'clauses: for clause in self.problem.clauses_containing_lit(lit) {
                match clause.eval_with_partial(&self.assignment) {
                    ClauseState::Satisfied => continue 'clauses, // 1 clause satisfied => check next
                    ClauseState::Unsatisfied => {
                        return PropagationResult::UNSAT; // Conflict => backtrack
                    }
                    ClauseState::Undecided(_) => continue 'clauses, // continue checking for conflicts and unit clauses
                    ClauseState::Unit(unit_literal) => {
                        let var = unit_literal.var();
                        if self.assignment[var].is_assigned() {
                            // Check if the variable is already assigned the opposite value
                            if !self.assignment[var].is_bool(unit_literal.is_pos()) {
                                return PropagationResult::UNSAT; // Conflict => backtrack
                            }
                            // Variable already assigned correctly, no action needed
                        } else {
                            // Variable is unassigned. Assign the variable such that the unit literal is true.
                            // => The unit clause will be satisfied.
                            self.assignment.propagate(var, unit_literal.is_pos());

                            // Unit literal is now true => its negation is false
                            self.falsified_lits_buffer.push(unit_literal.negated());
                        }
                    }
                }
            }
        }
        // All propagations done without conflicts.

        return if self.all_clauses_satisfied() {
            PropagationResult::SAT
        } else {
            PropagationResult::Undecided
        };
    }

    /// Makes a branching decision by selecting an unassigned variable and assigning it to true.
    pub fn make_branching_decision(&mut self) -> Lit {
        let decision_var = *self
            .decision_candidate_cursor
            .next_match(&self.problem.vars_by_score, |&var| {
                self.assignment[var].is_unassigned()
            });

        self.assignment.decide(decision_var);
        // Return the negated literal of the assigned decision variable
        return Lit::new(decision_var, true).negated();
    }

    fn all_clauses_satisfied(&self) -> bool {
        self.problem
            .clauses
            .iter()
            .all(|c| c.is_satisfied_by_partial(&self.assignment))
    }
}

enum PropagationResult {
    SAT,
    UNSAT,
    Undecided,
}

pub enum SolverAction {
    SAT,
    UNSAT,
    Continue(Lit),
}
