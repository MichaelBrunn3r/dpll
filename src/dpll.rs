use crate::{
    clause::{ClauseState, Lit},
    partial_assignment::{BacktrackResult, PartialAssignment},
    pool::DecisionPath,
    problem::Problem,
    vsids::VSIDS,
};

pub struct DPLLSolver<'a> {
    problem: &'a Problem,
    pub assignment: PartialAssignment,
    /// Reusable buffer for literals that have just been falsified during unit propagation.
    falsified_lits_buffer: Vec<Lit>,
    vsids: VSIDS,
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
            match self.step(falsified_lit) {
                SolverAction::SAT => {
                    return Some(self.assignment.to_solution());
                }
                SolverAction::Decision(next_falsified_lit) => {
                    falsified_lit = next_falsified_lit;
                }
                SolverAction::Backtrack => {
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
        match self.propagate_units(next_falsified_lit) {
            PropagationResult::SAT => {
                return SolverAction::SAT;
            }
            PropagationResult::UNSAT => {
                return SolverAction::Backtrack;
            }
            PropagationResult::Undecided => {
                // No conflicts & not all clauses satisfied => some clauses are still undecided
                // Make the next branching decision
                return SolverAction::Decision(self.make_branching_decision());
            }
        }
    }

    /// Performs unit propagation starting from the literal that was just falsified.
    fn propagate_units(&mut self, falsified_lit: Lit) -> PropagationResult {
        self.falsified_lits_buffer.clear();
        self.falsified_lits_buffer.push(falsified_lit);

        // Propagate until no unit clauses are left.
        // It's sufficient to only check clauses containing the just falsified literals,
        // since only those clauses can become unit clauses or conflicts.
        while let Some(lit) = self.falsified_lits_buffer.pop() {
            'clauses: for clause in self.problem.clauses_containing_lit(lit) {
                match clause.eval_with_partial(&self.assignment) {
                    ClauseState::Satisfied => continue 'clauses, // 1 clause satisfied => check next
                    ClauseState::Unsatisfied => {
                        self.vsids.bump_lit_activities(&clause.0);
                        self.vsids.decay();
                        return PropagationResult::UNSAT; // Conflict => backtrack
                    }
                    ClauseState::Undecided(_) => continue 'clauses, // continue checking for conflicts and unit clauses
                    ClauseState::Unit(unit_literal) => {
                        let var = unit_literal.var();
                        debug_assert!(
                            self.assignment[var].is_none(),
                            "Unit literal should be unassigned"
                        );
                        self.assignment.propagate(var, unit_literal.is_pos());
                        self.falsified_lits_buffer.push(unit_literal.inverted());
                    }
                }
            }
        }

        // No unit clauses left & we encountered no conflicts.
        // If all variables are assigned => all clauses must be satisfied => SAT.
        // Otherwise => Some clauses are still undecided.
        return if self.assignment.is_complete() {
            PropagationResult::SAT
        } else {
            PropagationResult::Undecided
        };
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

enum PropagationResult {
    SAT,
    UNSAT,
    Undecided,
}

pub enum SolverAction {
    SAT,
    Backtrack,
    Decision(Lit),
}
