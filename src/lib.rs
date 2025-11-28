pub mod parser;
pub mod utils;

use std::fmt;

// ----------------------
// DATA TYPES
// ----------------------

// Represents a literal as an integer.
// Even numbers are positive literals (v), Odd numbers are negative (!v).
// Var 0 -> Lit 0 (pos), Lit 1 (neg)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lit(u32);

impl Lit {
    #[inline]
    fn new(var: usize, is_pos: bool) -> Self {
        Lit((var as u32) << 1 | if is_pos { 0 } else { 1 })
    }

    #[inline]
    fn var(&self) -> usize {
        (self.0 >> 1) as usize
    }

    #[inline]
    fn is_pos(&self) -> bool {
        (self.0 & 1) == 0
    }

    #[inline]
    fn negate(&self) -> Self {
        Lit(self.0 ^ 1)
    }

    #[inline]
    fn to_usize(&self) -> usize {
        self.0 as usize
    }
}

// 0: Unassigned, 1: True, 2: False
// Using u8 is more cache-friendly than Option<bool>
#[derive(Clone, Copy, PartialEq, Eq)]
struct LBool(u8);

impl LBool {
    const UNDEF: LBool = LBool(0);
    const TRUE: LBool = LBool(1);
    const FALSE: LBool = LBool(2);

    // Convert boolean to LBool
    fn from_bool(b: bool) -> Self {
        if b { Self::TRUE } else { Self::FALSE }
    }

    // Check if satisfied by a specific boolean assignment
    fn is_satisfied_by(&self, val: bool) -> bool {
        match *self {
            Self::TRUE => val,
            Self::FALSE => !val,
            _ => false,
        }
    }
}

// ----------------------
// PROBLEM STRUCTURE
// ----------------------

pub struct Problem {
    pub num_vars: usize,

    // FLAT CLAUSE DATABASE
    // All literals from all clauses packed sequentially.
    clause_db: Vec<Lit>,
    // (start_index, length) in clause_db for each clause
    pub clauses: Vec<(u32, u32)>,

    // OCCURRENCE LIST (Adjacency List)
    // Map: Lit (as usize) -> List of Clause Indices that contain this literal.
    // This allows us to only visit relevant clauses during propagation.
    occurrences: Vec<Vec<usize>>,
}

impl Problem {
    pub fn new(num_vars: usize) -> Self {
        // Pre-allocate occurrences: 2 * num_vars (for pos and neg literals)
        let occurrences = vec![Vec::new(); num_vars * 2];
        Problem {
            num_vars,
            clause_db: Vec::new(),
            clauses: Vec::new(),
            occurrences,
        }
    }

    // Add a clause using a slice of (variable_index, is_positive)
    pub fn add_clause(&mut self, lits: &[(usize, bool)]) {
        let start = self.clause_db.len() as u32;
        let len = lits.len() as u32;
        let clause_idx = self.clauses.len();

        for &(var, is_pos) in lits {
            let lit = Lit::new(var, is_pos);
            self.clause_db.push(lit);

            // Add this clause to the occurrence list of the literal
            self.occurrences[lit.to_usize()].push(clause_idx);
        }

        self.clauses.push((start, len));
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
                while let Some((lit, tried_flipped, old_trail_len)) = stack.pop() {
                    // Undo trail
                    while trail.len() > old_trail_len {
                        let l = trail.pop().unwrap();
                        assignment[l.var()] = LBool::UNDEF;
                    }

                    if !tried_flipped {
                        // We tried 'lit' (e.g. True). Now try '!lit' (False).
                        let flipped = lit.negate();

                        // Set value
                        assignment[flipped.var()] = if flipped.is_pos() {
                            LBool::TRUE
                        } else {
                            LBool::FALSE
                        };
                        trail.push(flipped);

                        // Push flipped to stack
                        stack.push((flipped, true, old_trail_len));

                        // We must process this new assignment in propagation next loop
                        prop_queue.push(flipped);

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
            for clause_idx in 0..self.clauses.len() {
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
            let falsified_lit = just_assigned_true.negate();

            for &clause_idx in &self.occurrences[falsified_lit.to_usize()] {
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
        let current = assignment[lit.var()];

        if current == LBool::UNDEF {
            assignment[lit.var()] = val;
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
        let (start, len) = self.clauses[clause_idx];
        let slice = &self.clause_db[start as usize..(start + len) as usize];

        let mut unassigned_lit = None;
        let mut unassigned_count = 0;

        for &lit in slice {
            let is_pos = lit.is_pos();
            match assignment[lit.var()] {
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
                LBool::UNDEF | _ => {
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
        let (start, len) = self.clauses[clause_idx];
        let slice = &self.clause_db[start as usize..(start + len) as usize];

        // Conflict if ALL literals evaluate to false
        for &lit in slice {
            match assignment[lit.var()] {
                LBool::UNDEF => return false, // Not a conflict yet
                LBool::TRUE => {
                    if lit.is_pos() {
                        return false;
                    }
                } // Satisfied
                LBool::FALSE => {
                    if !lit.is_pos() {
                        return false;
                    }
                } // Satisfied
                _ => return false, // Treat unexpected numeric LBool values as UNDEF (not a conflict)
            }
        }
        true
    }
}
