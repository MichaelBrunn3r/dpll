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

pub fn solve_parallel(problem: Arc<Problem>, num_threads: usize) -> Option<Vec<bool>> {
    if num_threads <= 1 {
        return DPLLSolver::new(&problem).solve();
    }

    let job_queue = Arc::new(Mutex::new(generate_jobs(
        problem.num_vars,
        3.min(problem.num_vars),
    )));
    let solution_found = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();

    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let problem = Arc::clone(&problem);
        let queue = Arc::clone(&job_queue);
        let solution_flag = Arc::clone(&solution_found);
        let sender = tx.clone();

        handles.push(thread::spawn(move || {
            loop {
                if solution_flag.load(atomic::Ordering::SeqCst) {
                    break; // Solution already found
                }

                let job = match { queue.lock().unwrap().pop_front() } {
                    Some(job) => job,
                    None => break, // No more jobs
                };

                let problem = Arc::clone(&problem);
                let mut solver = DPLLSolver::with_assignment(&problem, job);
                if let Some(solution) = solver.solve() {
                    solution_flag.store(true, atomic::Ordering::SeqCst);
                    let _ = sender.send(solution);
                    break;
                }
            }
        }));
    }

    drop(tx);
    rx.recv().ok()
}

pub fn generate_jobs(num_vars: usize, depth: usize) -> VecDeque<Vec<Option<bool>>> {
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
