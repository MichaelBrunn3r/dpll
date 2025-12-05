use std::sync::atomic;

use crate::{
    dpll::DPLLSolver,
    pool::{Job, JobResult},
};

pub struct Worker {
    id: usize,
    job_receiver: crossbeam_channel::Receiver<Job>,
}

impl Worker {
    pub fn new(id: usize, job_receiver: crossbeam_channel::Receiver<Job>) -> Self {
        Worker { id, job_receiver }
    }

    pub fn run(&self) {
        while let Ok(mut job) = self.job_receiver.recv() {
            let mut solver = DPLLSolver::with_assignment(&job.problem, &mut job.init_assignment);
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
    }
}
