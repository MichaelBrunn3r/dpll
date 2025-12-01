use stackvector::StackVec;
use std::sync::atomic::{self, AtomicBool};

use crate::{
    clause::{ClauseState, Lit},
    constants::MAX_FALSIFIED_LITS,
    partial_assignment::{PartialAssignment, VarState},
    problem::Problem,
};

pub struct DPLLSolver<'a> {
    problem: &'a Problem,
    assignment: PartialAssignment<'a>,
    /// Reusable buffer for literals that have just been falsified during unit propagation.
    falsified_lits_buffer: StackVec<[Lit; MAX_FALSIFIED_LITS]>,
    /// Index to track the next candidate variable for branching decisions.
    next_decision_candidate_idx: usize,
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
            next_decision_candidate_idx: 0,
        }
    }

    pub fn solve(&mut self, abort_flag: &AtomicBool) -> Option<Vec<bool>> {
        let mut next_falsified_lit = self.make_branching_decision();

        'backtrack: loop {
            if abort_flag.load(atomic::Ordering::Relaxed) {
                return None;
            }

            match self.propagate_units(next_falsified_lit) {
                PropagationResult::Satisfied => {
                    return Some(self.assignment.to_solution());
                }
                PropagationResult::Unsatisfied => {
                    self.next_decision_candidate_idx = 0; // Reset decision candidate when backtracking
                    match self.assignment.backtrack() {
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
        self.falsified_lits_buffer.clear();
        self.falsified_lits_buffer.push(falsified_lit);

        // For each literal that was just falsified, check only the affected clauses.
        while let Some(lit) = self.falsified_lits_buffer.pop() {
            'clauses: for clause in self.problem.clauses_containing_lit(lit) {
                match clause.eval_with_partial(&self.assignment) {
                    ClauseState::Satisfied => continue 'clauses, // 1 clause satisfied => check next
                    ClauseState::Unsatisfied => {
                        return PropagationResult::Unsatisfied; // Conflict => backtrack
                    }
                    ClauseState::Undecided(_) => continue 'clauses, // continue checking for conflicts and unit clauses
                    ClauseState::Unit(unit_literal) => {
                        let var = unit_literal.var();
                        if self.assignment[var].is_assigned() {
                            // Check if the variable is already assigned the opposite value
                            if !self.assignment[var].is_bool(unit_literal.is_pos()) {
                                return PropagationResult::Unsatisfied; // Conflict => backtrack
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
            PropagationResult::Satisfied
        } else {
            PropagationResult::Undecided
        };
    }

    /// Makes a branching decision by selecting an unassigned variable and assigning it to true.
    fn make_branching_decision(&mut self) -> Lit {
        let decision_var = match self.find_var_with_highest_score() {
            Some(v) => v,
            None => {
                debug_assert!(
                    false,
                    "PropagationResult::Undecided implies some unassigned variable exists."
                );
                unreachable!()
            }
        };

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

    fn find_var_with_highest_score(&mut self) -> Option<usize> {
        for &var in &self.problem.vars_by_score[self.next_decision_candidate_idx..] {
            self.next_decision_candidate_idx += 1;

            if self.assignment[var].is_unassigned() {
                return Some(var);
            }
        }

        unreachable!("PropagationResult::Undecided implies some unassigned variable exists.")
    }
}

enum PropagationResult {
    Satisfied,
    Unsatisfied,
    Undecided,
}
