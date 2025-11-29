use crate::clause::{ClauseState, ClauseView, Lit};

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

        // Ignore tautological clauses
        if ClauseView::from(lits.as_slice()).is_tautology() {
            return;
        }

        let start = self.clause_literals.len() as u32;
        let len = lits.len() as u32;
        let clause_idx = self.clause_spans.len();

        // Add literals to the flat vector and update occurrences
        for lit in lits.drain(0..) {
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

    fn dpll(&self, initial_assignment: Vec<Option<bool>>) -> Option<Vec<bool>> {
        let mut stack = vec![initial_assignment];

        'backtrack: while let Some(mut assignment) = stack.pop() {
            // --- Phase 1: Unit propagation ---
            // Repeat until no unit clauses or conflicts are found
            'unit_prop: loop {
                let mut all_clauses_satisfied = true;
                let mut unit_lit: Option<Lit> = None;

                'find_unit: for clause in self.clauses() {
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

        for (i, clause) in self.clauses().enumerate() {
            if !clause.is_satisfied_by(solution) {
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
