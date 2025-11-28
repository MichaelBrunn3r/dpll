pub mod parser;
pub mod utils;
use bitvec::{bitvec, order::Lsb0, slice::BitSlice, vec::BitVec};
use std::{collections::HashSet, fmt};

#[derive(Debug, Clone)]
pub struct Problem {
    pub num_vars: usize,
    pub num_clauses: usize,
    arena_pos: BitVec<usize, Lsb0>,
    arena_neg: BitVec<usize, Lsb0>,
}

impl Problem {
    pub fn new(num_vars: usize, num_clauses: usize) -> Self {
        let total_bits = num_vars
            .checked_mul(num_clauses)
            .expect("Problem too large");
        Problem {
            num_vars,
            num_clauses,
            arena_pos: bitvec![usize, Lsb0; 0; total_bits],
            arena_neg: bitvec![usize, Lsb0; 0; total_bits],
        }
    }

    pub fn solve(&self) -> Option<Vec<bool>> {
        // Initialize all variables as unassigned (None)
        let assignment = vec![None; self.num_vars];
        self.dpll(assignment)
    }

    fn dpll(&self, mut assignment: Assignment) -> Option<Vec<bool>> {
        // 1. UNIT PROPAGATION
        // We loop until no more unit clauses are found
        loop {
            let mut made_change = false;
            let mut unit_lits = HashSet::new();

            // Scan all clauses to find units or conflicts
            for clause in self.clauses() {
                match self.evaluate_clause(clause, &assignment) {
                    ClauseStatus::Conflict => return None, // Backtrack immediately
                    ClauseStatus::Unit(var, val) => {
                        // Determine if we have a contradiction within this propagation step
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
        let all_satisfied = self.clauses().all(|c| {
            matches!(
                self.evaluate_clause(c, &assignment),
                ClauseStatus::Satisfied
            )
        });

        if all_satisfied {
            // Fill any remaining None values with default (false) and return
            return Some(assignment.into_iter().map(|x| x.unwrap_or(false)).collect());
        }

        // 3. BRANCHING (Heuristic: Pick first unassigned variable)
        // Find the first index that is None
        let branch_var = assignment.iter().position(|x| x.is_none());

        if let Some(var_idx) = branch_var {
            // Try True
            let mut left_branch = assignment.clone();
            left_branch[var_idx] = Some(true);
            if let Some(result) = self.dpll(left_branch) {
                return Some(result);
            }

            // Try False
            let mut right_branch = assignment; // Reuse the vector ownership
            right_branch[var_idx] = Some(false);
            return self.dpll(right_branch);
        }

        None
    }

    fn evaluate_clause(&self, clause: ClauseView, assignment: &Assignment) -> ClauseStatus {
        let mut unassigned_count = 0;
        let mut last_unassigned = None;

        // Check positive literals
        for i in clause.pos.iter_ones() {
            match assignment[i] {
                Some(true) => return ClauseStatus::Satisfied, // Clause is true
                Some(false) => continue,                      // Literal is false, keep looking
                None => {
                    unassigned_count += 1;
                    last_unassigned = Some((i, true)); // Needs True to satisfy
                }
            }
        }

        // Check negative literals
        for i in clause.neg.iter_ones() {
            match assignment[i] {
                Some(false) => return ClauseStatus::Satisfied, // Clause is true (Not False = True)
                Some(true) => continue, // Literal is false (Not True = False)
                None => {
                    unassigned_count += 1;
                    last_unassigned = Some((i, false)); // Needs False to satisfy
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

    /// Returns a view of the clause at the given index.
    pub fn clause<'p>(&'p self, index: usize) -> ClauseView<'p> {
        let start = index * self.num_vars;
        let end = start + self.num_vars;

        ClauseView {
            pos: &self.arena_pos[start..end],
            neg: &self.arena_neg[start..end],
        }
    }

    /// Returns an iterator over all clauses in the problem.
    pub fn clauses<'p>(&'p self) -> impl Iterator<Item = ClauseView<'p>> {
        (0..self.num_clauses).map(move |i| self.clause(i))
    }
}

type Assignment = Vec<Option<bool>>;

#[derive(Debug, PartialEq)]
enum ClauseStatus {
    Satisfied,
    Unresolved,
    Unit(usize, bool), // (Variable Index, Boolean Value to satisfy)
    Conflict,
}

#[derive(Debug, Clone, Copy)]
pub struct ClauseView<'a> {
    pub pos: &'a BitSlice<usize, Lsb0>,
    pub neg: &'a BitSlice<usize, Lsb0>,
}

impl<'a> fmt::Display for ClauseView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;

        for i in self.pos.iter_ones() {
            if !first {
                write!(f, " ∨ ")?;
            }
            write!(f, "{}", i + 1)?;
            first = false;
        }

        for i in self.neg.iter_ones() {
            if !first {
                write!(f, " ∨ ")?;
            }
            write!(f, "¬{}", i + 1)?;
            first = false;
        }

        Ok(())
    }
}
