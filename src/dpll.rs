use crate::{
    clause::Clause,
    lit::Lit,
    partial_assignment::{BacktrackResult, PartialAssignment},
    pool::cube_and_conquer::DecisionPath,
    problem::Problem,
    vsids::VSIDS,
};

pub struct DPLLSolver<'a> {
    problem: &'a Problem,
    pub assignment: PartialAssignment,
    /// Reusable buffer for literals that have just been falsified during unit propagation.
    falsified_lits_buffer: Vec<Lit>,
    pub vsids: VSIDS,
}

impl<'a> DPLLSolver<'a> {
    pub fn with_decisions(problem: &'a Problem, initial_decisions: &DecisionPath) -> Self {
        DPLLSolver {
            problem,
            assignment: PartialAssignment::with_decisions(problem.num_vars, initial_decisions),
            falsified_lits_buffer: Vec::new(),
            vsids: VSIDS::with_scores(&problem.var_scores),
        }
    }

    pub fn solve(&mut self) -> Option<Vec<bool>> {
        let mut falsified_lit = self.make_branching_decision();
        loop {
            use SolverAction::*;
            match self.step(falsified_lit) {
                SAT => {
                    return Some(self.assignment.to_solution());
                }
                Decision(next_falsified_lit) => {
                    falsified_lit = next_falsified_lit;
                }
                Backtrack => {
                    match self
                        .assignment
                        .backtrack(|var| self.vsids.on_unassign_var(var))
                    {
                        None => {
                            // Cannot backtrack further => UNSAT
                            return None;
                        }
                        Some(new_falsified_lit) => {
                            falsified_lit = new_falsified_lit;
                        }
                    }
                }
            }
        }
    }

    pub fn step(&mut self, next_falsified_lit: Lit) -> SolverAction {
        use UnitPropagationResult::*;
        match self.propagate_units_from(next_falsified_lit) {
            SAT => {
                return SolverAction::SAT;
            }
            UNSAT => {
                return SolverAction::Backtrack;
            }
            Undecided => {
                // No conflicts & not all clauses satisfied => some clauses are still undecided
                // Make the next branching decision
                return SolverAction::Decision(self.make_branching_decision());
            }
        }
    }

    /// Performs unit propagation when no decisions have been made yet.
    pub fn propagate_units_root(&mut self) -> UnitPropagationResult {
        self.falsified_lits_buffer.clear();

        // Make 1 full pass over all clauses to find an initial set of unit clauses.
        if self.propagate_clauses(&self.problem.clauses) == UnitPropagationResult::UNSAT {
            return UnitPropagationResult::UNSAT;
        }

        self.propagate_falsified_lits()
    }

    /// Performs unit propagation starting from the literal that was just falsified.
    pub fn propagate_units_from(&mut self, falsified_lit: Lit) -> UnitPropagationResult {
        self.falsified_lits_buffer.clear();
        self.falsified_lits_buffer.push(falsified_lit);
        self.propagate_falsified_lits()
    }

    /// Performs unit propagation for all literals in the falsified literals buffer.
    fn propagate_falsified_lits(&mut self) -> UnitPropagationResult {
        // Propagate until no unit clauses are left.
        // It's sufficient to only check clauses containing the just falsified literals,
        // since only those clauses can become unit clauses or conflicts.
        while let Some(lit) = self.falsified_lits_buffer.pop() {
            let clauses = self.problem.clauses_containing_lit(lit);
            if self.propagate_clauses(clauses) == UnitPropagationResult::UNSAT {
                return UnitPropagationResult::UNSAT;
            }
        }

        // No unit clauses left & we encountered no conflicts.
        // If all variables are assigned => all clauses must be satisfied => SAT.
        // Otherwise => Some clauses are still undecided.
        if self.assignment.is_complete() {
            UnitPropagationResult::SAT
        } else {
            UnitPropagationResult::Undecided
        }
    }

    /// Propagates the given clauses, adding any newly falsified literals to the falsified literals buffer.
    fn propagate_clauses<'s, I>(&'s mut self, clauses: I) -> UnitPropagationResult
    where
        I: IntoIterator<Item = &'s Clause>,
    {
        for clause in clauses {
            use crate::clause::ClauseState::*;
            match clause.eval_with_partial(&self.assignment) {
                Satisfied => continue, // 1 clause satisfied => check next
                Unsatisfied => {
                    self.vsids.bump_lit_activities(&clause.0);
                    self.vsids.decay();
                    return UnitPropagationResult::UNSAT; // Conflict => backtrack
                }
                Undecided(_) => continue, // continue checking for conflicts and unit clauses
                Unit(unit_literal) => {
                    let var = unit_literal.var();
                    self.assignment.propagate(var, unit_literal.is_pos());
                    self.falsified_lits_buffer.push(unit_literal.inverted());
                }
            }
        }

        UnitPropagationResult::Undecided
    }

    pub fn backtrack_one_level(&mut self) -> BacktrackResult {
        self.assignment.backtrack_once(|var| {
            self.vsids.on_unassign_var(var);
        })
    }

    /// Makes a branching decision by selecting an unassigned variable and assigning it to true.
    pub fn make_branching_decision(&mut self) -> Lit {
        let decision_var = self
            .vsids
            .pop_most_active_unassigned_var(&self.assignment)
            .expect("Called make_branching_decision but all variables are assigned");

        self.assignment.decide(decision_var);
        // Return the negated literal of the assigned decision variable
        return Lit::new(decision_var, true).inverted();
    }
}

/// Next action for the DPLL solver to take after a step.
pub enum SolverAction {
    SAT,
    Backtrack,
    Decision(Lit),
}

/// Result of a unit propagation step.
#[derive(Debug, PartialEq, Eq)]
pub enum UnitPropagationResult {
    SAT,
    UNSAT,
    Undecided,
}
