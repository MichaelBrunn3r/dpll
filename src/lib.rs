pub mod parser;
pub mod utils;

use fixedbitset::FixedBitSet;
use std::{collections::HashSet, fmt};

#[derive(Debug, Clone)]
pub struct Problem {
    pub num_vars: usize,
    pub num_clauses: usize,
    pub clauses: Vec<Clause>,
}

impl Problem {
    pub fn new(num_vars: usize, num_clauses: usize) -> Self {
        Problem {
            num_vars,
            num_clauses,
            clauses: Vec::with_capacity(num_clauses),
        }
    }

    pub fn solve(&self) -> Option<Vec<bool>> {
        // Initialize all variables as unassigned (None)
        let assignment = vec![None; self.num_vars];
        self.dpll(assignment)
    }

    fn dpll(&self, mut assignment: Assignment) -> Option<Vec<bool>> {
        // 1. UNIT PROPAGATION
        loop {
            let mut made_change = false;
            let mut unit_lits = HashSet::new();

            for clause in &self.clauses {
                match self.evaluate_clause(clause, &assignment) {
                    ClauseStatus::Conflict => return None, // Backtrack immediately
                    ClauseStatus::Unit(var, val) => {
                        if let Some(existing) = assignment[var] {
                            if existing != val {
                                return None;
                            } // Conflict on same var
                        } else {
                            unit_lits.insert((var, val));
                        }
                    }
                    _ => {}
                }
            }

            // Apply unit literals
            for (var, val) in unit_lits {
                if assignment[var].is_none() {
                    assignment[var] = Some(val);
                    made_change = true;
                }
            }

            if !made_change {
                break;
            }
        }

        // 2. CHECK IF ALL CLAUSES SATISFIED
        let all_satisfied = self.clauses.iter().all(|c| {
            matches!(
                self.evaluate_clause(c, &assignment),
                ClauseStatus::Satisfied
            )
        });

        if all_satisfied {
            return Some(assignment.into_iter().map(|x| x.unwrap_or(false)).collect());
        }

        // 3. BRANCHING (Heuristic: Pick first unassigned variable)
        let branch_var = assignment.iter().position(|x| x.is_none());

        if let Some(var_idx) = branch_var {
            // Try True
            let mut left_branch = assignment.clone();
            left_branch[var_idx] = Some(true);
            if let Some(result) = self.dpll(left_branch) {
                return Some(result);
            }

            // Try False
            let mut right_branch = assignment;
            right_branch[var_idx] = Some(false);
            return self.dpll(right_branch);
        }

        None
    }

    fn evaluate_clause(&self, clause: &Clause, assignment: &Assignment) -> ClauseStatus {
        let mut unassigned_count = 0;
        let mut last_unassigned = None;
        let num_vars = self.num_vars;

        // Iterate over set bits (literals present in the clause)
        for i in clause.literals.ones() {
            // Determine variable index and polarity based on bit position
            // 0..num_vars -> Positive (true)
            // num_vars..2*num_vars -> Negative (false)
            let (var_idx, is_positive_literal) = if i < num_vars {
                (i, true)
            } else {
                (i - num_vars, false)
            };

            match assignment[var_idx] {
                Some(val) => {
                    // If the assignment matches the literal's polarity, the clause is satisfied.
                    // (e.g. var is true, literal is positive -> true == true -> satisfied)
                    // (e.g. var is false, literal is negative -> false == false -> satisfied)
                    if val == is_positive_literal {
                        return ClauseStatus::Satisfied;
                    }
                    // If val != is_positive_literal, this literal evaluates to false.
                    // We simply continue to the next literal.
                }
                None => {
                    unassigned_count += 1;
                    // To satisfy this literal, the variable must match the literal's polarity
                    last_unassigned = Some((var_idx, is_positive_literal));
                }
            }
        }

        match unassigned_count {
            0 => ClauseStatus::Conflict,
            1 => {
                let (var, val) = last_unassigned.unwrap();
                ClauseStatus::Unit(var, val)
            }
            _ => ClauseStatus::Unresolved,
        }
    }
}

type Assignment = Vec<Option<bool>>;

#[derive(Debug, Clone)]
pub struct Clause {
    /// A single bitset storing both positive and negative literals.
    /// Indices 0..num_vars represent positive literals.
    /// Indices num_vars..2*num_vars represent negative literals.
    pub literals: FixedBitSet,
}

impl Clause {
    pub fn new(size: usize) -> Self {
        Self {
            literals: FixedBitSet::with_capacity(size),
        }
    }
}

impl fmt::Display for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We infer num_vars from the bitset size (it's always constructed as 2 * num_vars)
        let num_vars = self.literals.len() / 2;
        let mut first = true;

        for i in self.literals.ones() {
            if !first {
                write!(f, " ∨ ")?;
            }

            if i < num_vars {
                // Positive literal
                write!(f, "{}", i + 1)?;
            } else {
                // Negative literal
                write!(f, "¬{}", i + 1 - num_vars)?;
            }
            first = false;
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
enum ClauseStatus {
    Satisfied,
    Unresolved,
    Unit(usize, bool),
    Conflict,
}
