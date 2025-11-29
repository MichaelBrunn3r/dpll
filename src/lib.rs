use crate::clause::{ClauseView, Lit};

pub mod clause;
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

        if ClauseView::from(lits.as_slice()).is_tautology() {
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
        let assignment = vec![None; self.num_vars];
        self.dpll(assignment)
    }

    fn dpll(&self, mut assignment: Vec<Option<bool>>) -> Option<Vec<bool>> {
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
                // ...existing code...
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
                        assignment[l.var_index()] = None;
                    }

                    if !tried_negated {
                        // We tried 'lit' (e.g. True). Now try '!lit' (False).
                        let negated = lit.negated();

                        // Set value
                        assignment[negated.var_index()] = if negated.is_pos() {
                            Some(true)
                        } else {
                            Some(false)
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
                    if assignment[v].is_none() {
                        pick = Some(v);
                        break;
                    }
                }

                match pick {
                    None => {
                        // All assigned, no conflict -> SAT
                        return Some(assignment.iter().map(|&x| x == Some(true)).collect());
                    }
                    Some(var) => {
                        // Branching: heuristic (try TRUE first)
                        let lit = Lit::new(var, true);
                        let trail_len = trail.len();

                        assignment[var] = Some(true);
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
        assignment: &mut Vec<Option<bool>>,
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
                let clause = self.clause_at(clause_idx);
                if let Some(l) = clause.find_unit_literal(assignment.as_slice()) {
                    if self.apply_lit(l, assignment, trail, queue) {
                        return true;
                    }
                } else if clause.conflicts_with(assignment.as_slice()) {
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
                let clause = self.clause_at(clause_idx);
                if let Some(unit_lit) = clause.find_unit_literal(assignment.as_slice()) {
                    // Found a unit!
                    if self.apply_lit(unit_lit, assignment, trail, queue) {
                        return true; // Conflict
                    }
                } else if clause.conflicts_with(assignment.as_slice()) {
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
        assignment: &mut Vec<Option<bool>>,
        trail: &mut Vec<Lit>,
        queue: &mut Vec<Lit>,
    ) -> bool {
        let val = if lit.is_pos() {
            Some(true)
        } else {
            Some(false)
        };
        let current = assignment[lit.var_index()];

        if current.is_none() {
            assignment[lit.var_index()] = val;
            trail.push(lit);
            queue.push(lit);
            false
        } else {
            current != val // Conflict if assigned opposite
        }
    }

    /// Verifies if the given assignment satisfies all clauses in the problem.
    pub fn verify(&self, assignment: &[bool]) -> Result<(), String> {
        debug_assert_eq!(
            assignment.len(),
            self.num_vars,
            "Assignment length does not match number of variables."
        );

        for (i, clause) in self.clauses().enumerate() {
            if !clause.satisfied_by(assignment) {
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
        ClauseView::from(&self.clause_literals[span.start as usize..span.end()])
    }

    /// Returns an iterator over views of all clauses in the problem.
    pub fn clauses(&self) -> impl Iterator<Item = ClauseView<'_>> {
        self.clause_spans.iter().map(move |span| {
            ClauseView::from(&self.clause_literals[span.start as usize..span.end()])
        })
    }
}

/// Identifier for a clause that is unique within a Problem.
type ClauseID = usize;

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
