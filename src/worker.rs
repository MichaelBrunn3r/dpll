use std::sync::{Arc, RwLock, atomic};

use crate::{
    dpll::{DPLLSolver, SolverAction},
    pool::{ProblemContext, SharedContext, Task, WorkerResult},
    utils::opt_bool::OptBool,
};

pub struct Worker {
    id: usize,
    task_receiver: crossbeam_channel::Receiver<Task>,

    // Shared context across all workers with local caching
    shared_ctx: Arc<RwLock<SharedContext>>,
    local_pid: usize,
    local_problem_ctx: Arc<ProblemContext>,
}

impl Worker {
    pub fn new(
        id: usize,
        msg_receiver: crossbeam_channel::Receiver<Task>,
        shared_ctx: Arc<RwLock<SharedContext>>,
    ) -> Self {
        Worker {
            id,
            task_receiver: msg_receiver,
            shared_ctx,
            local_pid: 0,
            local_problem_ctx: Arc::new(ProblemContext::default()),
        }
    }

    pub fn run(&mut self) {
        while let Ok(mut task) = self.task_receiver.recv() {
            // Check if the problem context has changed
            if task.pid != self.local_pid {
                if let Ok(guard) = self.shared_ctx.read() {
                    if guard.current_pid == task.pid {
                        self.local_problem_ctx = guard.problem_ctx.clone();
                        self.local_pid = guard.current_pid;
                    }
                }
            }

            // Solve the assigned task
            self.solve(&self.local_problem_ctx, &mut task.init_assignment);
        }
    }

    pub fn solve(&self, ctx: &ProblemContext, init_assignment: &mut Vec<OptBool>) {
        let mut solver = DPLLSolver::with_assignment(&ctx.problem, init_assignment);

        let mut falsified_lit = solver.make_branching_decision();
        loop {
            if ctx.solution_found_flag.load(atomic::Ordering::Relaxed) {
                // Another worker has found a solution, stop working on this job
                break;
            }

            match solver.step(falsified_lit) {
                SolverAction::SAT => {
                    // Signal other workers to stop working on this job
                    ctx.solution_found_flag
                        .store(true, atomic::Ordering::Release);

                    // Send the found solution
                    let _ = ctx
                        .sender
                        .send(WorkerResult::SAT(solver.assignment.to_solution()));
                    break;
                }
                SolverAction::UNSAT => {
                    // Sub-problem is UNSAT
                    let _ = ctx.sender.send(WorkerResult::UNSAT);
                    break;
                }
                SolverAction::Continue(next_falsified_lit) => {
                    falsified_lit = next_falsified_lit;
                }
            }
        }
    }
}
