use stackvector::StackVec;

use crate::{
    clause::{Clause, Lit, VariableId},
    constants::{MAX_CLAUSES_PER_LIT, MAX_CLAUSES_PER_VAR},
};

#[derive(Debug, Clone)]
pub struct Problem {
    pub num_vars: usize,
    pub clauses: Vec<Clause>,
    /// Maps each variable to the list of clauses it appears in.
    pub var2clauses: Vec<StackVec<[ClauseID; MAX_CLAUSES_PER_VAR]>>,
    /// Maps each literal to the list of clauses it appears in.
    pub lit2clauses: Vec<StackVec<[ClauseID; MAX_CLAUSES_PER_LIT]>>,
    /// Jeroslow-Wang scores for each variable.
    pub var_scores: Vec<f64>,
    /// Variables sorted by their Jeroslow-Wang scores in descending order.
    pub vars_by_score: Vec<VariableId>,
}

impl Problem {
    pub fn new(num_vars: usize, clauses: Vec<Clause>) -> Self {
        let var_scores = Self::calculate_jeroslow_wang_scores(&clauses, num_vars);

        let mut vars_by_score: Vec<usize> = (0..num_vars).collect();
        vars_by_score.sort_by(|&a, &b| {
            var_scores[b]
                .partial_cmp(&var_scores[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut lit2clauses = vec![StackVec::new(); num_vars * 2];
        let mut var2clauses = vec![StackVec::new(); num_vars];

        for (clause_id, clause) in clauses.iter().enumerate() {
            for lit in &clause.0 {
                lit2clauses[lit.0 as usize].push(clause_id);
                var2clauses[lit.var()].push(clause_id);
            }
        }

        Problem {
            num_vars,
            clauses,
            var2clauses,
            lit2clauses,
            var_scores,
            vars_by_score,
        }
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

    pub fn clauses_containing_lit(&self, lit: Lit) -> impl Iterator<Item = &Clause> {
        let clause_ids = &self.lit2clauses[lit.0 as usize];
        clause_ids.iter().map(|&id| &self.clauses[id])
    }

    pub fn clauses_containing_var(&self, var_id: usize) -> impl Iterator<Item = &Clause> {
        let clause_ids = &self.var2clauses[var_id];
        clause_ids.iter().map(|&id| &self.clauses[id])
    }

    /// Calculates the Jeroslow-Wang scores for all variables in the problem.
    pub fn calculate_jeroslow_wang_scores(clauses: &[Clause], num_vars: usize) -> Vec<f64> {
        let mut var_scores = vec![0.0; num_vars];

        for clause in clauses {
            // Weight of the clause is 2^(-|clause|)
            let clause_weight = 2f64.powf(-(clause.0.len() as f64));

            // Add the clause weight to each variable in the clause
            for lit in &clause.0 {
                var_scores[lit.var()] += clause_weight;
            }
        }
        var_scores
    }
}

impl Default for Problem {
    fn default() -> Self {
        Problem {
            num_vars: 0,
            clauses: Vec::new(),
            var2clauses: Vec::new(),
            lit2clauses: Vec::new(),
            var_scores: Vec::new(),
            vars_by_score: Vec::new(),
        }
    }
}

/// Identifier for a clause that is unique within a Problem.
type ClauseID = usize;

pub struct ProblemBuilder {
    num_vars: usize,
    clauses: Vec<Clause>,
}

impl ProblemBuilder {
    pub fn new(num_vars: usize, num_clauses: usize) -> Self {
        ProblemBuilder {
            num_vars,
            clauses: Vec::with_capacity(num_clauses),
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

        self.clauses.push(clause.clone());
    }

    pub fn build(self) -> Problem {
        Problem::new(self.num_vars, self.clauses)
    }
}
