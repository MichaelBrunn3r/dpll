use std::sync::atomic;

use crate::{
    dpll::DPLLSolver,
    partial_assignment::VarState,
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
        let mut assignment_buffer = Vec::new();
        while let Ok(job) = self.job_receiver.recv() {
            // Ensure buffer capacity
            if assignment_buffer.len() < job.problem.num_vars {
                assignment_buffer.resize(job.problem.num_vars, VarState::new_unassigned());
            }

            // Reconstruct partial assignment from combination
            assignment_buffer[0..job.problem.num_vars].fill(VarState::new_unassigned());
            for (bit_idx, &var_idx) in job.split_vars.iter().enumerate() {
                let val = (job.combination & (1 << bit_idx)) != 0;
                assignment_buffer[var_idx] = VarState::new_assigned(val);
            }

            let mut solver = DPLLSolver::with_assignment(
                &job.problem,
                &mut assignment_buffer[..job.problem.num_vars],
            );
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
