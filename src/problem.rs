use crate::clause::{Clause, Lit};

pub struct Problem {
    pub num_vars: usize,
    pub clauses: Vec<Clause>,
    /// Maps each variable to the list of clauses it appears in.
    pub var2clauses: Vec<Vec<ClauseID>>,
    /// Maps each literal to the list of clauses it appears in.
    pub lit2clauses: Vec<Vec<ClauseID>>,
}

impl Problem {
    pub fn new(num_vars: usize, num_clauses: usize) -> Self {
        Problem {
            num_vars,
            clauses: Vec::with_capacity(num_clauses),
            var2clauses: vec![Vec::new(); num_vars],
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
            self.var2clauses[lit.var()].push(clause_id);
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

    pub fn clauses_containing_lit(&self, lit: Lit) -> impl Iterator<Item = &Clause> {
        let clause_ids = &self.lit2clauses[lit.0 as usize];
        clause_ids.iter().map(|&id| &self.clauses[id])
    }

    pub fn clauses_containing_var(&self, var_id: usize) -> impl Iterator<Item = &Clause> {
        let clause_ids = &self.var2clauses[var_id];
        clause_ids.iter().map(|&id| &self.clauses[id])
    }

    /// Calculates the Jeroslow-Wang scores for all variables in the problem.
    pub fn calculate_jeroslow_wang_scores(&self) -> Vec<f64> {
        let mut var_scores = vec![0.0; self.num_vars];

        for clause in &self.clauses {
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

/// Identifier for a clause that is unique within a Problem.
type ClauseID = usize;
