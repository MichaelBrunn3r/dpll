use std::sync::atomic;

use crate::{
    dpll::{DPLLSolver, SolverAction},
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
            self.solve(&mut job);
        }
    }

    pub fn solve(&self, job: &mut Job) {
        let mut solver = DPLLSolver::with_assignment(&job.problem, &mut job.init_assignment);

        let mut falsified_lit = solver.make_branching_decision();
        loop {
            if job.solution_found_flag.load(atomic::Ordering::Relaxed) {
                // Another worker has found a solution, stop working on this job
                break;
            }

            match solver.step(falsified_lit) {
                SolverAction::SAT => {
                    // Signal other workers to stop working on this job
                    job.solution_found_flag
                        .store(true, atomic::Ordering::Release);

                    // Send the found solution
                    let _ = job
                        .sender
                        .send(JobResult::Found(solver.assignment.to_solution()));
                    break;
                }
                SolverAction::UNSAT => {
                    // Sub-problem is UNSAT
                    let _ = job.sender.send(JobResult::Done);
                    break;
                }
                SolverAction::Continue(next_falsified_lit) => {
                    falsified_lit = next_falsified_lit;
                }
            }
        }
    }
}
