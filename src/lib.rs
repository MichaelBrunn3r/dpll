#![feature(iter_from_coroutine)]
#![feature(coroutines)]
#![feature(yield_expr)]

use crate::{dpll::DPLLSolver, problem::Problem};
use itertools::Itertools;
use std::{
    sync::{
        Arc,
        atomic::{self, AtomicBool},
        mpsc,
    },
    thread::{self, available_parallelism},
};

pub mod clause;
pub mod dpll;
pub mod parser;
pub mod partial_assignment;
pub mod problem;
pub mod utils;

pub struct SolverPool {
    job_sender: Option<crossbeam_channel::Sender<Job>>,
    _workers: Vec<thread::JoinHandle<()>>,
    pub num_workers: usize,
}

impl SolverPool {
    pub fn new(num_workers: usize) -> Self {
        if num_workers <= 1 {
            return Self {
                job_sender: None,
                _workers: Vec::new(),
                num_workers: 1,
            };
        }
        // Limit number of workers to available parallelism
        let num_workers = num_workers.min(available_parallelism().map(|n| n.get()).unwrap_or(1));

        let (tx, rx) = crossbeam_channel::unbounded::<Job>();
        let mut workers = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let rx = rx.clone();
            workers.push(thread::spawn(move || {
                loop {
                    let job = match rx.recv() {
                        Ok(job) => job,
                        _ => break, // Channel closed, stop the worker
                    };

                    let mut assignment = vec![None; job.problem.num_vars];
                    for (var, val) in &job.decision {
                        assignment[*var] = Some(*val);
                    }

                    let mut solver = DPLLSolver::with_assignment(&job.problem, assignment);
                    match solver.solve(&job.solution_found_flag) {
                        Some(solution) => {
                            // Signal other workers to stop working on this job
                            job.solution_found_flag
                                .store(true, atomic::Ordering::Release);
                            // Send the found solution
                            let _ = job.sender.send(JobResult::Found(solution));
                        }
                        None => {
                            // No solution found for this job
                            let _ = job.sender.send(JobResult::Done);
                        }
                    }
                }
            }));
        }

        Self {
            job_sender: Some(tx),
            _workers: workers,
            num_workers,
        }
    }

    pub fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        let job_sender = match &self.job_sender {
            Some(tx) => tx,
            None => {
                // Single-threaded mode
                let mut solver = DPLLSolver::new(&problem);
                return solver.solve(&AtomicBool::new(false));
            }
        };

        let solution_found = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let depth = Self::calculate_depth(self.num_workers, problem.num_vars);
        let split_vars = Self::select_split_vars(&problem, depth);

        let mut active_jobs = 0;
        for decision in Self::generate_decision(&problem, split_vars, &solution_found) {
            let job = Job {
                problem: Arc::clone(&problem),
                solution_found_flag: Arc::clone(&solution_found),
                sender: tx.clone(),
                decision,
            };

            if job_sender.send(job).is_err() {
                return None; // Failed to send job
            }

            active_jobs += 1;
        }

        drop(tx);

        if active_jobs == 0 {
            return None; // No valid jobs were generated, i.e. problem is unsatisfiable
        }

        let mut completed_jobs = 0;
        while let Ok(result) = rx.recv() {
            match result {
                JobResult::Found(solution) => return Some(solution),
                JobResult::Done => {
                    completed_jobs += 1;
                    if completed_jobs == active_jobs {
                        return None; // All jobs completed, no solution found
                    }
                }
            }
        }

        None
    }

    /// Calculate depth required to generate at least `num_workers` initial assignments.
    pub fn calculate_depth(num_workers: usize, num_vars: usize) -> usize {
        ((num_workers as f64).log2().ceil() as usize).min(num_vars)
    }

    fn select_split_vars(problem: &Problem, depth: usize) -> Vec<usize> {
        // Sort variables by score in descending order
        let sorted_vars = problem
            .var_scores
            .iter()
            .enumerate()
            .map(|(var, &score)| (var, score))
            .sorted_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take the variables with the highest scores
        sorted_vars.take(depth).map(|(var, _)| var).collect()
    }

    /// Generates all possible assignments for the given split variables,
    /// skipping those that lead to immediate unsatisfied clauses.
    fn generate_decision<'p>(
        problem: &'p Problem,
        split_vars: Vec<usize>,
        solution_found: &'p AtomicBool,
    ) -> impl Iterator<Item = Vec<(usize, bool)>> + 'p {
        let clauses_containing_split_vars = split_vars
            .iter()
            .flat_map(|&var| problem.clauses_containing_var(var))
            .unique_by(|c| *c as *const _)
            .collect::<Vec<_>>();

        let num_vars = problem.num_vars;
        let total_combinations = 1usize << split_vars.len();

        generator!(move || {
            let mut assignment = vec![None; num_vars];
            for i in 0..total_combinations {
                // Check if a solution has been found since we started generating
                if solution_found.load(atomic::Ordering::Relaxed) {
                    return; // Stop generating!
                }

                for (bit_idx, &var) in split_vars.iter().enumerate() {
                    let val = (i & (1 << bit_idx)) != 0;
                    assignment[var] = Some(val);
                }

                // Check if any clause containing split vars is unsatisfied
                if clauses_containing_split_vars
                    .iter()
                    .any(|clause| clause.is_unsatisfied_by_partial(&assignment))
                {
                    continue; // Skip this assignment as it leads to unsatisfied clauses
                }

                let decision = assignment
                    .iter()
                    .enumerate()
                    .filter_map(|(var, &val)| val.map(|v| (var, v)))
                    .collect::<Vec<_>>();

                yield decision
            }
        })
    }
}

enum JobResult {
    /// A solution was found for this job.
    Found(Vec<bool>),
    /// No solution found for this job.
    Done,
}

/// A job to be processed by a worker.
struct Job {
    problem: Arc<Problem>,
    solution_found_flag: Arc<AtomicBool>,
    sender: mpsc::Sender<JobResult>,
    decision: Vec<(usize, bool)>,
}
