use crate::clause::{Clause, ClauseState, Lit};

pub mod clause;
pub mod parser;
pub mod utils;

pub struct Problem {
    pub num_vars: usize,
    pub clauses: Vec<Clause>,
    /// Maps each literal to the list of clauses it appears in.
    lit2clauses: Vec<Vec<ClauseID>>,
}

impl Problem {
    pub fn new(num_vars: usize, num_clauses: usize) -> Self {
        Problem {
            num_vars,
            clauses: Vec::with_capacity(num_clauses),
            lit2clauses: vec![Vec::new(); num_vars * 2], // Each variable can be positive or negated
        }
    }

    pub fn add_clause(&mut self, clause: &mut Clause) {
        // Ensure literals are unique and sorted
        clause.0.sort_unstable();
        clause.0.dedup();

        // Ignore tautological clauses
        if clause.is_tautology() {
            return;
        }

        let clause_id = self.clauses.len();
        for lit in &clause.0 {
            self.lit2clauses[lit.0 as usize].push(clause_id);
        }

        self.clauses.push(clause.clone());
    }

    /// Verifies if the given assignment satisfies all clauses in the problem.
    pub fn verify_solution(&self, solution: &[bool]) -> Result<(), String> {
        debug_assert_eq!(
            solution.len(),
            self.num_vars,
            "Assignment length does not match number of variables."
        );

        for (i, clause) in self.clauses.iter().enumerate() {
            if !clause.is_satisfied_by(solution) {
                return Err(format!("Clause {} is unsatisfied.", i));
            }
        }

        Ok(())
    }
}

pub struct DPLLSolver<'p> {
    problem: &'p Problem,
    assignment: Vec<Option<bool>>,
    /// History of variable assignments for backtracking
    assign_history: Vec<VariableAssignment>,
    // History of decision levels for backtracking. Stores indices into the 'assign_history'
    // where each decision level starts. Required, because in each decision level, multiple
    // forced variable assignments (unit propagations) may occur.
    decision_history: Vec<usize>,
    decision_level: usize,
}

impl<'p> DPLLSolver<'p> {
    pub fn new(problem: &'p Problem) -> Self {
        let num_vars = problem.num_vars;
        DPLLSolver {
            problem,
            assignment: vec![None; num_vars],
            decision_history: vec![0],
            assign_history: Vec::new(),
            decision_level: 0,
        }
    }

    pub fn solve(&mut self) -> Option<Vec<bool>> {
        'backtrack: loop {
            // --- Phase 1: Unit propagation ---
            // Repeat until no unit clauses or conflicts are found
            'unit_prop: loop {
                let mut all_clauses_satisfied = true;
                let mut unit_lit: Option<Lit> = None;

                'find_unit: for clause in &self.problem.clauses {
                    match clause.eval_with(&self.assignment) {
                        ClauseState::Satisfied => continue 'find_unit, // 1 clause satisfied => check next
                        ClauseState::Undecided(_) => {
                            all_clauses_satisfied = false;
                            continue 'find_unit; // continue checking for conflicts and unit clauses
                        }
                        ClauseState::Unit(unit_literal) => {
                            // Found a unit clause => forced to assign the unit literal
                            all_clauses_satisfied = false;
                            unit_lit = Some(unit_literal);
                            break 'find_unit;
                        }
                        ClauseState::Unsatisfied => {
                            if !self.backtrack() {
                                return None; // Unsatisfiable => return UNSAT
                            }
                            continue 'backtrack; // Backtracked => restart search
                        }
                    }
                }

                if all_clauses_satisfied {
                    return Some(solution_from_assignment(&self.assignment)); // All clauses satisfied => return the solution
                }

                if let Some(lit) = unit_lit {
                    // Assign the unit literal to the value that satisfies the unit clause

                    self.assign_history.push(VariableAssignment {
                        var_id: lit.var_id(),
                        old_value: self.assignment[lit.var_id()],
                    });
                    self.assignment[lit.var_id()] = Some(lit.is_pos());

                    continue 'unit_prop;
                } else {
                    break 'unit_prop; // No unit clause or conflict => proceed to branching
                }
            }

            // --- Phase 2: Splitting / Branching ---

            // Find first unassigned variable
            let decision_var_id = if let Some(var_id) = self
                .assignment
                .iter()
                .enumerate()
                .find(|(_, val)| val.is_none())
                .map(|(id, _)| id)
            {
                var_id
            } else {
                return Some(solution_from_assignment(&self.assignment)); // All variables assigned => return the solution
            };

            self.decision_level += 1;
            self.decision_history.push(self.assign_history.len());

            self.assign_history.push(VariableAssignment {
                var_id: decision_var_id,
                old_value: self.assignment[decision_var_id],
            });
            self.assignment[decision_var_id] = Some(true); // First try assigning 'true'
        }
    }

    fn backtrack(&mut self) -> bool {
        if self.decision_level == 0 {
            // Cannot backtrack further => UNSAT
            return false;
        }

        self.reset_current_decision_level(self.decision_level);
        self.decision_level -= 1;

        loop {
            if self.try_flip_previous_decision() {
                return true;
            }
            // Flip failed, backtrack further

            if self.decision_level == 0 {
                return false; // Cannot backtrack further => UNSAT
            }

            // Backtrack one more level
            self.reset_current_decision_level(self.decision_level);
            self.decision_level -= 1;
        }
    }

    fn reset_current_decision_level(&mut self, decision_level: usize) {
        let start_of_current_level = self.decision_history[decision_level];

        // Revert all assignments made in the current decision level
        while self.assign_history.len() > start_of_current_level {
            if let Some(assignment_to_revert) = self.assign_history.pop() {
                self.assignment[assignment_to_revert.var_id] = assignment_to_revert.old_value;
            }
        }

        self.decision_history.pop();
    }

    fn try_flip_previous_decision(&mut self) -> bool {
        // Attempt to flip the previous decision variable.
        // Decision order: true -> false -> backtrack further
        if let Some(prev_decision) = self.assign_history.last_mut() {
            if self.assignment[prev_decision.var_id] == Some(false) {
                // Decision was already 'false'. Revert and backtrack further.
                self.assignment[prev_decision.var_id] = prev_decision.old_value;
                self.assign_history.pop();
                return false; // Flip failed
            } else {
                // Already tried 'true'. Now try 'false'.
                self.assignment[prev_decision.var_id] = Some(false);
                return true; // Flip successful
            }
        }
        false // No previous decision to flip
    }
}

/// Identifier for a clause that is unique within a Problem.
type ClauseID = usize;

#[derive(Debug)]
struct VariableAssignment {
    /// The ID of the variable that was assigned a value.
    var_id: usize,
    /// The previous value of the variable before assignment.
    old_value: Option<bool>,
}

fn solution_from_assignment(assignment: &[Option<bool>]) -> Vec<bool> {
    assignment.iter().map(|&val| val.unwrap_or(false)).collect()
}
