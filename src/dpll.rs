use crate::{
    clause::{ClauseState, Lit},
    partial_assignment::PartialAssignment,
    problem::Problem,
};

pub struct DPLLSolver<'p> {
    problem: &'p Problem,
    assignment: PartialAssignment,
    /// Pre-calculated Jeroslow-Wang for each variable.
    var_scores: Vec<f64>,
    /// Reusable buffer to store literals that become falsified during unit propagation.
    falsified_lits_buffer: Vec<Lit>,
}

impl<'p> DPLLSolver<'p> {
    pub fn new(problem: &'p Problem) -> Self {
        let num_vars = problem.num_vars;
        DPLLSolver {
            problem,
            assignment: PartialAssignment::new(num_vars),
            var_scores: problem.calculate_jeroslow_wang_scores(),
            falsified_lits_buffer: Vec::new(),
        }
    }

    pub fn with_assignment(problem: &'p Problem, initial_assignment: Vec<Option<bool>>) -> Self {
        debug_assert!(
            initial_assignment.len() == problem.num_vars,
            "Initial assignment length must match number of variables."
        );

        DPLLSolver {
            problem,
            assignment: PartialAssignment::with_assignment(initial_assignment),
            var_scores: problem.calculate_jeroslow_wang_scores(),
            falsified_lits_buffer: Vec::new(),
        }
    }

    pub fn solve(&mut self) -> Option<Vec<bool>> {
        let mut next_falsified_lit = self.make_branching_decision();

        'backtrack: loop {
            match self.propagate_units(next_falsified_lit) {
                PropagationResult::Satisfied => {
                    return Some(self.assignment.to_solution());
                }
                PropagationResult::Unsatisfied => {
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
                match clause.eval_with(&self.assignment) {
                    ClauseState::Satisfied => continue 'clauses, // 1 clause satisfied => check next
                    ClauseState::Unsatisfied => {
                        return PropagationResult::Unsatisfied; // Conflict => backtrack
                    }
                    ClauseState::Undecided(_) => continue 'clauses, // continue checking for conflicts and unit clauses
                    ClauseState::Unit(unit_literal) => {
                        let var = unit_literal.var();
                        if let Some(val) = self.assignment[var] {
                            // Check if the variable is already assigned the opposite value
                            if val != unit_literal.is_pos() {
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
            .all(|c| matches!(c.eval_with(&self.assignment), ClauseState::Satisfied))
    }

    // --- Heuristics ---

    fn find_var_with_highest_score(&self) -> Option<usize> {
        let mut max_score = f64::MIN;
        let mut best_var = None;

        for var in 0..self.problem.num_vars {
            if self.assignment[var].is_none() {
                let score = self.var_scores[var];
                if score > max_score {
                    max_score = score;
                    best_var = Some(var);
                }
            }
        }

        best_var
    }

    #[allow(dead_code)]
    fn find_most_frequent_var_in_undecided_clauses(&self) -> Option<usize> {
        let mut max_count = 0;
        let mut most_freq_var = None;

        for var in 0..self.problem.num_vars {
            if self.assignment[var].is_none() {
                let count = self
                    .problem
                    .clauses_containing_var(var)
                    .filter(|c| {
                        matches!(
                            c.eval_with(&self.assignment),
                            ClauseState::Unit(_) | ClauseState::Undecided(_)
                        )
                    })
                    .count();

                if count > max_count {
                    max_count = count;
                    most_freq_var = Some(var);
                }
            }
        }

        most_freq_var
    }
}

enum PropagationResult {
    Satisfied,
    Unsatisfied,
    Undecided,
}
