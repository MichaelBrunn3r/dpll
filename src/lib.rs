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

    pub fn solve(&self) -> Option<Vec<bool>> {
        let assignment = vec![None; self.num_vars];
        self.dpll(assignment)
    }

    fn dpll(&self, initial_assignment: Vec<Option<bool>>) -> Option<Vec<bool>> {
        let mut stack = vec![initial_assignment];

        'backtrack: while let Some(mut assignment) = stack.pop() {
            // --- Phase 1: Unit propagation ---
            // Repeat until no unit clauses or conflicts are found
            'unit_prop: loop {
                let mut all_clauses_satisfied = true;
                let mut unit_lit: Option<Lit> = None;

                'find_unit: for clause in &self.clauses {
                    match clause.eval_with(&assignment) {
                        ClauseState::Unsatisfied => continue 'backtrack, // conflict => backtrack
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
                    }
                }

                if all_clauses_satisfied {
                    // All clauses satisfied => return the solution
                    return Some(
                        assignment
                            .into_iter()
                            .map(|val| val.unwrap_or(false))
                            .collect(),
                    );
                }

                if let Some(lit) = unit_lit {
                    // Assign the unit literal to the value that satisfies the unit clause
                    assignment[lit.var_id()] = Some(lit.is_pos());
                    continue 'unit_prop; // Continue unit propagation until no unit clauses remain
                } else {
                    break 'unit_prop; // No unit clause or conflict => proceed to branching
                }
            }

            // --- Phase 2: Splitting / Branching ---

            let decision_var_idx = if let Some(idx) =
                assignment.iter().position(|val| val.is_none())
            {
                idx
            } else {
                eprint!(
                    "Error: All variables assigned but not all clauses satisfied. This should be caught in the undecided check above."
                );
                continue 'backtrack; // Don't panic, just backtrack
            };

            // Branch 1: Assign decision variable to false
            let mut assignment_false = assignment.clone();
            assignment_false[decision_var_idx] = Some(false);
            stack.push(assignment_false);

            // Branch 2: Assign decision variable to true
            assignment[decision_var_idx] = Some(true);
            stack.push(assignment);
        }

        None
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

/// Identifier for a clause that is unique within a Problem.
type ClauseID = usize;
