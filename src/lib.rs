use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{self, AtomicBool},
        mpsc,
    },
    thread,
};

use crate::{dpll::DPLLSolver, problem::Problem};

pub mod clause;
pub mod dpll;
pub mod parser;
pub mod partial_assignment;
pub mod problem;
pub mod utils;

pub struct SolverPool {
    job_sender: mpsc::Sender<Job>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl SolverPool {
    pub fn new(num_workers: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let mut workers = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let rx = rx.clone();
            workers.push(thread::spawn(move || {
                loop {
                    let job = match rx.lock().unwrap().recv() {
                        Ok(job) => job,
                        _ => break, // Channel closed, stop the worker
                    };

                    if job.solution_found_flag.load(atomic::Ordering::Relaxed) {
                        // Notify that we are "done" (skipped)
                        let _ = job.sender.send(JobResult::Done);
                        continue;
                    }

                    let mut solver = DPLLSolver::with_assignment(&job.problem, job.assignment);
                    match solver.solve() {
                        Some(solution) => {
                            // Signal other workers to stop working on this job
                            job.solution_found_flag
                                .store(true, atomic::Ordering::Relaxed);
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
            job_sender: tx,
            _workers: workers,
        }
    }

    pub fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        let solution_found = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let initial_assignments =
            generate_initial_assignments(problem.num_vars, 3.min(problem.num_vars));
        let num_jobs = initial_assignments.len();
        for assignment in initial_assignments {
            let job = Job {
                problem: Arc::clone(&problem),
                assignment,
                solution_found_flag: Arc::clone(&solution_found),
                sender: tx.clone(),
            };

            if self.job_sender.send(job).is_err() {
                return None; // Failed to send job
            }
        }

        drop(tx);

        let mut completed_jobs = 0;
        while let Ok(result) = rx.recv() {
            match result {
                JobResult::Found(solution) => return Some(solution),
                JobResult::Done => {
                    completed_jobs += 1;
                    if completed_jobs == num_jobs {
                        return None; // All jobs completed, no solution found
                    }
                }
            }
        }

        None
    }
}

pub fn generate_initial_assignments(num_vars: usize, depth: usize) -> VecDeque<Vec<Option<bool>>> {
    let mut initial_assignments: VecDeque<Vec<Option<bool>>> = VecDeque::new();

    let mut assignment = 0usize;
    for _ in 0..(1 << depth) {
        let mut assignment_vec = vec![None; num_vars];
        for i in 0..depth {
            if (assignment & (1 << i)) != 0 {
                assignment_vec[i] = Some(true);
            } else {
                assignment_vec[i] = Some(false);
            }
        }
        initial_assignments.push_back(assignment_vec);
        assignment += 1;
    }

    initial_assignments
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
    assignment: Vec<Option<bool>>,
    solution_found_flag: Arc<AtomicBool>,
    sender: mpsc::Sender<JobResult>,
}
