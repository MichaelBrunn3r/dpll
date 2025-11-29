pub mod parser;
pub mod utils;

pub struct Problem {
    pub num_vars: usize,

    /// A flat vector storing all clause literals sequentially. Literals are sorted and unique within each clause.
    /// E.g. clauses `1 -3`, `2  3 -1` are stored as \[+1, -3, -1, +2, +3\]
    clause_literals: Vec<Lit>,

    /// Spans (start, length) for each clause in the flat literals vector.
    /// E.g. for the above example, we have two spans: (0,2) and (2,3)
    clause_spans: Vec<ClauseSpan>,

    /// Maps each literal to the list of clauses it appears in.
    lit_occurrences: Vec<Vec<ClauseID>>,
}

impl Problem {
    pub fn new(num_vars: usize, num_clauses: usize) -> Self {
        Problem {
            num_vars,
            clause_literals: Vec::new(),
            clause_spans: Vec::with_capacity(num_clauses),
            lit_occurrences: vec![Vec::new(); num_vars * 2], // Each var has pos and neg literal
        }
    }

    pub fn add_clause(&mut self, lits: &mut Vec<Lit>) {
        // Ensure literals are unique and sorted
        lits.sort_unstable();
        lits.dedup();

        if is_tautology(lits) {
            return; // Ignore tautological clauses
        }

        let start = self.clause_literals.len() as u32;
        let len = lits.len() as u32;
        let clause_idx = self.clause_spans.len();

        for &lit in lits.iter() {
            self.clause_literals.push(lit);

            // Add this clause to the occurrence list of the literal
            self.lit_occurrences[lit.0 as usize].push(clause_idx);
        }

        self.clause_spans.push(ClauseSpan { start, len });
    }

    pub fn solve(&self) -> Option<Vec<bool>> {
        // Flat vector is faster than Vec<Option<bool>>
        let assignment = vec![LBool::UNDEF; self.num_vars];
        self.dpll(assignment)
    }

    fn dpll(&self, mut assignment: Vec<LBool>) -> Option<Vec<bool>> {
        let mut trail: Vec<Lit> = Vec::with_capacity(self.num_vars);

        // Stack stores: (decision_lit, tried_both_branches_flag, trail_len_before_decision)
        // We store the literal we decided on (e.g., +x). If we backtrack, we try -x.
        let mut stack: Vec<(Lit, bool, usize)> = Vec::new();

        // Queue for propagation (BFS style)
        let mut prop_queue: Vec<Lit> = Vec::with_capacity(self.num_vars);

        loop {
            // 1. UNIT PROPAGATION
            // We pass the queue to avoid reallocation, but we must populate it first
            // if we just made a decision.
            if !trail.is_empty() {
                // Optimization: Only propagate the implications of the most recent assignments.
                // In a basic DPLL, we often just scan everything or use the queue.
                // Here, we'll use a queue-based approach.
            }

            let conflict = self.propagate(&mut assignment, &mut trail, &mut prop_queue);

            if conflict {
                if stack.is_empty() {
                    return None;
                }

                let mut backtracked = false;
                while let Some((lit, tried_negated, old_trail_len)) = stack.pop() {
                    // Undo trail
                    while trail.len() > old_trail_len {
                        let l = trail.pop().unwrap();
                        assignment[l.var_index()] = LBool::UNDEF;
                    }

                    if !tried_negated {
                        // We tried 'lit' (e.g. True). Now try '!lit' (False).
                        let negated = lit.negated();

                        // Set value
                        assignment[negated.var_index()] = if negated.is_pos() {
                            LBool::TRUE
                        } else {
                            LBool::FALSE
                        };
                        trail.push(negated);

                        // Push negated to stack
                        stack.push((negated, true, old_trail_len));

                        // We must process this new assignment in propagation next loop
                        prop_queue.push(negated);

                        backtracked = true;
                        break;
                    }
                }

                if !backtracked {
                    return None;
                }
            } else {
                // 2. CHECK SUCCESS / PICK BRANCH
                // Pick first undefined variable
                let mut pick = None;
                for v in 0..self.num_vars {
                    if assignment[v] == LBool::UNDEF {
                        pick = Some(v);
                        break;
                    }
                }

                match pick {
                    None => {
                        // All assigned, no conflict -> SAT
                        return Some(assignment.iter().map(|&x| x == LBool::TRUE).collect());
                    }
                    Some(var) => {
                        // Branching: heuristic (try TRUE first)
                        let lit = Lit::new(var, true);
                        let trail_len = trail.len();

                        assignment[var] = LBool::TRUE;
                        trail.push(lit);
                        stack.push((lit, false, trail_len));

                        // Add to propagation queue
                        prop_queue.push(lit);
                    }
                }
            }
        }
    }

    // Returns true if conflict found
    fn propagate(
        &self,
        assignment: &mut Vec<LBool>,
        trail: &mut Vec<Lit>,
        queue: &mut Vec<Lit>, // queue contains literals newly satisfied (assigned true)
    ) -> bool {
        // If this is the start and queue is empty, we might need to scan all clauses (initial pass)
        // or rely on the caller to fill queue.
        // For simplicity in this DPLL refactor:
        // We will scan clauses containing !L for every L in the queue.
        // (Because if L is True, !L is False, so clauses with !L might become Unit).

        // NOTE: In a pure DPLL without watched literals, we usually scan ALL clauses.
        // But using the 'occurrences' map, we only scan relevant clauses.

        // If queue is empty (fresh start), we might have to scan everything once?
        // Actually, pure DPLL usually scans everything. Let's do the hybrid:
        // If queue is empty but trail is not, fill queue from trail?
        // For this snippet, let's assume queue is populated by the decision engine.

        // EDGE CASE: Initial Propagation (before any decisions)
        if queue.is_empty() && trail.is_empty() {
            // Scan all clauses once to find initial units
            for clause_idx in 0..self.clause_spans.len() {
                if let Some(l) = self.check_clause(clause_idx, assignment) {
                    if self.apply_lit(l, assignment, trail, queue) {
                        return true;
                    }
                } else if self.is_clause_conflict(clause_idx, assignment) {
                    return true;
                }
            }
            return false;
        }

        let mut ptr = 0;
        while ptr < queue.len() {
            let just_assigned_true = queue[ptr];
            ptr += 1;

            // We need to check clauses that contain the NEGATION of what was just assigned.
            // Why? Because those clauses just lost a literal (it became false).
            // Clauses containing 'just_assigned_true' are already satisfied.
            let falsified_lit = just_assigned_true.negated();

            for &clause_idx in &self.lit_occurrences[falsified_lit.0 as usize] {
                if let Some(unit_lit) = self.check_clause(clause_idx, assignment) {
                    // Found a unit!
                    if self.apply_lit(unit_lit, assignment, trail, queue) {
                        return true; // Conflict
                    }
                } else if self.is_clause_conflict(clause_idx, assignment) {
                    return true;
                }
            }
        }

        queue.clear();
        false
    }

    // Helper to assign a literal and detect conflicts
    fn apply_lit(
        &self,
        lit: Lit,
        assignment: &mut Vec<LBool>,
        trail: &mut Vec<Lit>,
        queue: &mut Vec<Lit>,
    ) -> bool {
        let val = if lit.is_pos() {
            LBool::TRUE
        } else {
            LBool::FALSE
        };
        let current = assignment[lit.var_index()];

        if current == LBool::UNDEF {
            assignment[lit.var_index()] = val;
            trail.push(lit);
            queue.push(lit);
            false
        } else {
            current != val // Conflict if assigned opposite
        }
    }

    // Returns Some(lit) if the clause is Unit(lit), None otherwise (Satisfied or Unresolved).
    // Conflict detection handled separately or implied if returns None but is all false.
    fn check_clause(&self, clause_idx: usize, assignment: &[LBool]) -> Option<Lit> {
        let clause = self.clause_at(clause_idx);

        let mut unassigned_lit = None;
        let mut unassigned_count = 0;

        for &lit in clause {
            let is_pos = lit.is_pos();
            match assignment[lit.var_index()] {
                LBool::TRUE => {
                    if is_pos {
                        return None;
                    }
                } // Clause Satisfied
                LBool::FALSE => {
                    if !is_pos {
                        return None;
                    }
                } // Clause Satisfied
                LBool::UNDEF => {
                    // Treat any unexpected numeric LBool value as UNDEF (unassigned)
                    unassigned_count += 1;
                    unassigned_lit = Some(lit);
                }
            }
        }

        if unassigned_count == 1 {
            unassigned_lit
        } else {
            None
        }
    }
    fn is_clause_conflict(&self, clause_idx: usize, assignment: &[LBool]) -> bool {
        let clause = self.clause_at(clause_idx);

        // Conflict if ALL literals evaluate to false
        for &lit in clause {
            match assignment[lit.var_index()] {
                LBool::UNDEF => return false, // Not a conflict yet
                LBool::TRUE => {
                    if lit.is_pos() {
                        return false;
                    }
                }
                LBool::FALSE => {
                    if !lit.is_pos() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Verifies if the given assignment satisfies all clauses in the problem.
    pub fn verify(&self, assignment: &[bool]) -> Result<(), String> {
        if assignment.len() != self.num_vars {
            return Err("Assignment length mismatch".to_string());
        }

        for (i, clause) in self.clauses().enumerate() {
            let mut clause_satisfied = false;

            for lit in clause {
                // Get the boolean value assigned to this variable
                let var_val = assignment[lit.var_index()];

                // Check if the literal evaluates to true
                // lit.is_pos() returns true for X, false for !X
                // If is_pos matches the assignment (True==True or False==False), the literal is true.
                if lit.is_pos() == var_val {
                    clause_satisfied = true;
                    break; // Optimization: One true literal satisfies the clause
                }
            }

            if !clause_satisfied {
                return Err(format!("Clause {} is unsatisfied.", i));
            }
        }

        Ok(())
    }

    pub fn num_clauses(&self) -> usize {
        self.clause_spans.len()
    }

    /// Returns a view of the clause at the specified index.
    pub fn clause_at<'a>(&'a self, clause_idx: usize) -> ClauseView<'a> {
        let span = &self.clause_spans[clause_idx];
        &self.clause_literals[span.start as usize..span.end()]
    }

    /// Returns an iterator over views of all clauses in the problem.
    pub fn clauses(&self) -> impl Iterator<Item = ClauseView<'_>> {
        self.clause_spans
            .iter()
            .map(move |span| &self.clause_literals[span.start as usize..span.end()])
    }
}

// Represents a literal as an integer.
// Even numbers are positive literals (v), Odd numbers are negative (!v).
// Var 0 -> Lit 0 (pos), Lit 1 (neg)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lit(u32);

impl Lit {
    #[inline]
    fn new(var: usize, is_pos: bool) -> Self {
        Lit((var as u32) << 1 | (!is_pos as u32))
    }

    /// Returns the variable index (0-based).
    fn var_index(&self) -> usize {
        (self.0 >> 1) as usize
    }

    /// Returns true if the literal is positive.
    fn is_pos(&self) -> bool {
        (self.0 & 1) == 0
    }

    /// Returns the negation of the literal.
    fn negated(&self) -> Self {
        Lit(self.0 ^ 1)
    }
}

impl From<i32> for Lit {
    fn from(value: i32) -> Self {
        let var = value.abs() as usize - 1;
        let is_pos = value > 0;
        Lit::new(var, is_pos)
    }
}

impl std::fmt::Display for Lit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_pos() {
            write!(f, "{}", self.var_index() + 1)
        } else {
            write!(f, "¬{}", self.var_index() + 1)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum LBool {
    UNDEF = 0,
    TRUE = 1,
    FALSE = 2,
}

/// Identifier for a clause that is unique within a Problem.
type ClauseID = usize;

/// A view of a clauses literals.
pub type ClauseView<'a> = &'a [Lit];

/// Checks if a clause is a tautology (contains both a literal and its negation).
/// Assumes the clause is sorted and contains unique literals.
fn is_tautology(clause: ClauseView) -> bool {
    for i in 0..clause.len().saturating_sub(1) {
        if clause[i].var_index() == clause[i + 1].var_index() {
            return true;
        }
    }
    false
}

/// Span (start, length) of a clause within a flat clause literals array.
#[derive(Clone, Copy)]
struct ClauseSpan {
    start: u32,
    len: u32,
}

impl ClauseSpan {
    #[inline]
    fn end(&self) -> usize {
        (self.start + self.len as u32) as usize
    }
}

#[cfg(test)]
mod tests {

    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn test_lit_order() {
        let cases: Vec<(i32, i32, Ordering)> = vec![
            (1, 2, Ordering::Less),
            (2, 1, Ordering::Greater),
            (1, -1, Ordering::Less),
            (-1, 1, Ordering::Greater),
            (-2, -1, Ordering::Greater),
            (-1, -2, Ordering::Less),
            (3, 3, Ordering::Equal),
            (-3, -3, Ordering::Equal),
        ];

        for (a, b, expected) in cases {
            let lit_a = Lit::from(a);
            let lit_b = Lit::from(b);
            assert_eq!(lit_a.cmp(&lit_b), expected, "Comparing {} and {}", a, b);
        }
    }

    #[test]
    fn test_is_tautology() {
        let cases: Vec<(Vec<i32>, bool)> = vec![
            (vec![1, -1], true),
            (vec![1, 2, -2], true),
            (vec![-3, 3, 4], true),
            (vec![1, 2, 3], false),
            (vec![-1, -2, -3], false),
            (vec![1, -2, 3], false),
            (vec![], false),
        ];

        for (clause_ints, expected) in cases {
            let mut clause: Vec<Lit> = clause_ints.iter().map(|&x| Lit::from(x)).collect();
            clause.sort_unstable();
            clause.dedup();

            assert_eq!(
                is_tautology(&clause),
                expected,
                "Tautology check failed for clause {:?}",
                clause_ints
            );
        }
    }
}
