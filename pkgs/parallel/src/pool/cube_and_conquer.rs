use crate::generator;
use dpll_core::{DPLLSolver, DecisionPath, Problem};
use std::num::NonZeroUsize;

pub struct CubeGenerator<'p> {
    problem: &'p Problem,
    max_depth: NonZeroUsize,
}

impl<'p> CubeGenerator<'p> {
    pub fn new(problem: &'p Problem, max_depth: NonZeroUsize) -> Self {
        Self { problem, max_depth }
    }

    pub fn generate(self) -> impl Iterator<Item = CubeGenerationResult> + 'p {
        return generator!(move || {
            let mut generated_any = false;
            let empty_path = DecisionPath(vec![]);
            let mut solver = DPLLSolver::with_decisions(self.problem, &empty_path);

            // 1. Root level unit propagation
            use dpll_core::UnitPropagationResult::*;
            match solver.propagate_units_root() {
                SAT => {
                    yield CubeGenerationResult::SAT(solver.assignment.to_solution());
                    return;
                }
                UNSAT => {
                    yield CubeGenerationResult::UNSAT;
                    return;
                }
                Undecided => {}
            }

            // 2. Make the first decision
            if let Some(var) = solver
                .vsids
                .pop_most_active_unassigned_var(&solver.assignment)
            {
                solver.assignment.decide(var);
            } else {
                // All variables assigned after root unit propagation
                // => trivial problem that was solved by forced assignments
                yield CubeGenerationResult::SAT(solver.assignment.to_solution());
                return;
            }

            loop {
                let falsified_lit = solver.assignment.last_decision_lit().unwrap().inverted();
                let abandon_branch = match solver.propagate_units_from(falsified_lit) {
                    SAT => {
                        yield CubeGenerationResult::SAT(solver.assignment.to_solution());
                        return;
                    }
                    UNSAT => true, // Conflict => This branch is a dead end
                    Undecided => {
                        if solver.assignment.decision_level() >= self.max_depth.get() {
                            // Reached max depth => save cube and threat branch as leaf
                            let mut cube = Vec::new();
                            solver.assignment.extract_decisions(&mut cube);
                            yield CubeGenerationResult::Cube(DecisionPath(cube));
                            generated_any = true;
                            true
                        } else {
                            false
                        }
                    }
                };

                if !abandon_branch {
                    if let Some(next_var) = solver
                        .vsids
                        .pop_most_active_unassigned_var(&solver.assignment)
                    {
                        // Make the next decision and continue down the tree
                        solver.assignment.decide(next_var);
                        continue;
                    } else {
                        // All variables assigned but no SAT found => dead end
                        // falls through to backtracking
                    }
                }

                // Backtrack to explore other branches
                loop {
                    use dpll_core::BacktrackResult::*;
                    match solver.backtrack_one_level() {
                        ContinueBacktracking => {} // Explored both branches of this decision => continue backtracking
                        TryAlternative(_) => break, // Flipped last decision => now trying the alternative branch
                        NoMoreDecisions => {
                            // Explored entire tree
                            if !generated_any {
                                yield CubeGenerationResult::UNSAT; // No cubes generated => problem is UNSAT
                            }
                            return;
                        }
                    }
                }
            }
        });
    }
}

pub enum CubeGenerationResult {
    /// Satisfying assignment found during cube generation.
    SAT(Vec<bool>),
    /// Problem determined to be unsatisfiable during cube generation.
    UNSAT,
    /// Generated a decision path (cube).
    Cube(DecisionPath),
}

#[cfg(test)]
mod tests {
    use dpll_core::{Clause, Lit};

    use super::*;
    use fastrand::Rng;
    use std::collections::HashSet;

    #[test]
    fn test_equivalence_on_random_3sat() {
        let mut rng = Rng::new();
        println!("Seed: {}", rng.get_seed());

        let num_vars = 60;
        let num_clauses = (num_vars as f64 * 4.26) as usize;
        let cutoff_depth = NonZeroUsize::new(10).unwrap();
        println!(
            "#Vars: {}, #Clauses: {}, Cutoff depth: {}",
            num_vars, num_clauses, cutoff_depth
        );

        let mut num_sat = 0;
        let mut num_unsat = 0;
        while num_sat < 10 || num_unsat < 10 {
            println!(
                "--- Iteration {} (Vars: {}, Clauses: {}) ---",
                num_sat + num_unsat + 1,
                num_vars,
                num_clauses
            );

            let problem = generate_random_3sat(num_vars, num_clauses, &mut rng);

            // Create ground truth solution
            let expected_solution =
                DPLLSolver::with_decisions(&problem, &DecisionPath(vec![])).solve();

            let is_sat = expected_solution.is_some();
            if is_sat {
                assert!(
                    problem
                        .verify_solution(expected_solution.as_ref().unwrap())
                        .is_ok(),
                    "Reference solver returned invalid SAT solution"
                );
                num_sat += 1;
            } else {
                num_unsat += 1;
            }

            println!("Ground truth: {}", if is_sat { "SAT" } else { "UNSAT" });

            // Generate cubes
            let generator = CubeGenerator::new(&problem, cutoff_depth);
            let (sat_solution, unsat, cubes) = exhaust_generator(generator.generate());

            // Verify cubes against ground truth
            if let Some(sol) = sat_solution {
                assert!(is_sat, "Expected UNSAT, but generator returned SAT");
                assert_eq!(
                    expected_solution.as_ref().unwrap().len(),
                    sol.len(),
                    "SAT solution lengths do not match"
                );
                assert!(
                    problem.verify_solution(&sol).is_ok(),
                    "Generator returned invalid SAT solution"
                );
            } else if unsat {
                assert!(!is_sat, "Expected SAT, but generator returned UNSAT");
            } else {
                println!("Generator produced {} cubes.", cubes.len());
                assert!(
                    !cubes.is_empty(),
                    "Should have generated cubes or returned SAT/UNSAT directly"
                );

                let mut found_sat_via_cubes = false;
                for cube in cubes.iter() {
                    let mut sub_solver = DPLLSolver::with_decisions(&problem, cube);
                    if let Some(_) = sub_solver.solve() {
                        found_sat_via_cubes = true;
                        break;
                    }
                }

                assert_eq!(
                    found_sat_via_cubes, is_sat,
                    "Reference SAT: {}, Cubes found SAT: {}",
                    is_sat, found_sat_via_cubes
                );
            }
        }
    }

    fn exhaust_generator(
        generator: impl Iterator<Item = CubeGenerationResult>,
    ) -> (Option<Vec<bool>>, bool, Vec<DecisionPath>) {
        let mut cubes = Vec::new();
        let mut sat = None;
        let mut unsat = false;

        for r in generator {
            match r {
                CubeGenerationResult::SAT(sol) => {
                    sat = Some(sol);
                    break;
                }
                CubeGenerationResult::UNSAT => {
                    unsat = true;
                    break;
                }
                CubeGenerationResult::Cube(c) => cubes.push(c),
            }
        }

        (sat, unsat, cubes)
    }

    fn generate_random_3sat(num_vars: usize, num_clauses: usize, rng: &mut Rng) -> Problem {
        let mut clauses = Vec::with_capacity(num_clauses);

        for _ in 0..num_clauses {
            let mut lits = Vec::with_capacity(3);
            let mut used_vars = HashSet::new();

            // Generate 3 distinct variables for the clause
            while lits.len() < 3 {
                let var = rng.usize(0..num_vars);
                if used_vars.contains(&var) {
                    continue;
                }
                used_vars.insert(var);
                let is_pos = rng.bool();
                lits.push(Lit::new(var, is_pos));
            }

            clauses.push(Clause(lits));
        }

        Problem::new(num_vars, clauses)
    }
}
