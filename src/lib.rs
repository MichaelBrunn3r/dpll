pub mod parser;
pub mod utils;

use fixedbitset::FixedBitSet;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Problem {
    pub num_vars: usize,
    pub num_clauses: usize,
    pub clauses: Vec<Clause>,
}

type Assignment = Vec<Option<bool>>;

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

    /// Iterative DPLL implementation using an explicit stack and trail for backtracking.
    fn dpll(&self, mut assignment: Assignment) -> Option<Vec<bool>> {
        // 'trail' keeps track of the chronological order of assignments so we can undo them.
        let mut trail: Vec<usize> = Vec::with_capacity(self.num_vars);

        // 'stack' replaces the recursion.
        // Stores: (decision_var_idx, tried_true_branch_already, trail_len_before_decision)
        let mut stack: Vec<(usize, bool, usize)> = Vec::new();

        // 'unit_buffer' is reused to store unit literals found during a single propagation pass.
        // Pre-allocating this prevents repeated allocations inside the inner loop.
        let mut unit_buffer: Vec<(usize, bool)> = Vec::with_capacity(self.num_vars);

        loop {
            // 1. UNIT PROPAGATION
            // Continually simplify clauses until no more units are found or conflict occurs
            let conflict_detected = self.propagate(&mut assignment, &mut trail, &mut unit_buffer);

            if conflict_detected {
                // If the stack is empty and we have a conflict, the problem is UNSAT.
                if stack.is_empty() {
                    return None;
                }

                // BACKTRACKING LOGIC
                let mut backtracked_successfully = false;

                // Pop decisions off the stack until we find a branch we haven't tried yet
                while let Some((var, tried_true, trail_start)) = stack.pop() {
                    // Undo all assignments made *after* the decision point was established
                    // This cleans up both unit propagations and the decision itself
                    while trail.len() > trail_start {
                        let v = trail.pop().unwrap();
                        assignment[v] = None;
                    }

                    if tried_true {
                        // We previously tried assigning `true`. Now we try `false`.
                        assignment[var] = Some(false);
                        trail.push(var); // Record this new assignment

                        // Push back onto stack noting that we are now on the second branch (tried_true = false)
                        // Note: We don't change trail_start, it remains the base for this level.
                        stack.push((var, false, trail_start));

                        backtracked_successfully = true;
                        break;
                    }
                    // If we already tried both branches (tried_true was false, or rather we are done with the second branch),
                    // we let the loop continue, effectively popping the NEXT decision up the stack.
                }

                if !backtracked_successfully {
                    return None; // Exhausted search space
                }
            } else {
                // 2. CHECK IF ALL CLAUSES SATISFIED
                // (Optimized: propagate returns conflict if fails, so we just check if full solution)
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
                    let trail_start = trail.len();

                    // Decision: Try True first
                    assignment[var_idx] = Some(true);
                    trail.push(var_idx);

                    // Push to stack: var_idx, we are trying 'true' (first branch), and the current trail mark
                    stack.push((var_idx, true, trail_start));
                } else {
                    // Should be covered by all_satisfied, but strictly speaking if no vars are None, we are done.
                    return Some(assignment.into_iter().map(|x| x.unwrap_or(false)).collect());
                }
            }
        }
    }

    /// Propagates unit clauses.
    /// Returns `true` if a conflict exists, `false` otherwise.
    /// Updates `assignment` and `trail` in place.
    fn propagate(
        &self,
        assignment: &mut Assignment,
        trail: &mut Vec<usize>,
        pending_units: &mut Vec<(usize, bool)>,
    ) -> bool {
        loop {
            let mut made_change = false;
            pending_units.clear(); // Reuse the buffer instead of allocating a new HashSet/Vec

            for clause in &self.clauses {
                match self.evaluate_clause(clause, assignment) {
                    ClauseStatus::Conflict => return true, // Immediate conflict
                    ClauseStatus::Unit(var, val) => {
                        // We found a unit literal.
                        // Note: evaluate_clause only returns Unit if the var is currently None.
                        pending_units.push((var, val));
                    }
                    _ => {}
                }
            }

            // Apply all discovered unit literals
            for &(var, val) in pending_units.iter() {
                match assignment[var] {
                    None => {
                        assignment[var] = Some(val);
                        trail.push(var);
                        made_change = true;
                    }
                    Some(existing) => {
                        // This can happen if two different clauses in the same pass
                        // imply opposite values for the same variable.
                        if existing != val {
                            return true; // Conflict
                        }
                    }
                }
            }

            if !made_change {
                return false; // Stable state reached, no conflicts
            }
        }
    }

    fn evaluate_clause(&self, clause: &Clause, assignment: &Assignment) -> ClauseStatus {
        let mut unassigned_count = 0;
        let mut last_unassigned = None;
        let num_vars = self.num_vars;

        for i in clause.literals.ones() {
            let (var_idx, is_positive_literal) = if i < num_vars {
                (i, true)
            } else {
                (i - num_vars, false)
            };

            match assignment[var_idx] {
                Some(val) => {
                    if val == is_positive_literal {
                        return ClauseStatus::Satisfied;
                    }
                }
                None => {
                    unassigned_count += 1;
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

#[derive(Debug, Clone)]
pub struct Clause {
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
        let num_vars = self.literals.len() / 2;
        let mut first = true;

        for i in self.literals.ones() {
            if !first {
                write!(f, " ∨ ")?;
            }
            if i < num_vars {
                write!(f, "{}", i + 1)?;
            } else {
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
