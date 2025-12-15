use std::sync::{
    Arc, RwLock,
    atomic::{self, AtomicUsize},
};

use crate::if_metrics;
use crate::{
    clause::Lit,
    dpll::{DPLLSolver, SolverAction},
    partial_assignment::BacktrackResult,
    pool::{ProblemContext, SharedContext, SubProblem},
    worker::{WorkerStrategy, metrics},
};

pub struct WorkerCore<S: WorkerStrategy> {
    _id: usize,
    num_active_workers: Arc<AtomicUsize>,
    subproblem_receiver: crossbeam_channel::Receiver<SubProblem>,
    shared_ctx: Arc<RwLock<SharedContext>>,
    cached_pid: usize,
    local_problem_ctx: Arc<ProblemContext>,
    strat: S,
}

impl<B: WorkerStrategy> WorkerCore<B> {
    pub fn new(
        id: usize,
        active_workers: Arc<AtomicUsize>,
        subproblem_receiver: crossbeam_channel::Receiver<SubProblem>,
        shared_ctx: Arc<RwLock<SharedContext>>,
        behavior: B,
    ) -> Self {
        WorkerCore {
            _id: id,
            subproblem_receiver,
            shared_ctx,
            strat: behavior,
            cached_pid: 0,
            local_problem_ctx: Arc::new(ProblemContext::default()),
            num_active_workers: active_workers,
        }
    }

    pub fn run(&mut self) {
        while let Ok(subproblem) = self.subproblem_receiver.recv() {
            if subproblem.pid != self.cached_pid {
                self.sync_problem_context(subproblem.pid);
            }

            self.num_active_workers
                .fetch_add(1, atomic::Ordering::Release);
            self.solve_subproblem(subproblem);
            self.num_active_workers
                .fetch_sub(1, atomic::Ordering::Release);
        }
    }

    pub fn solve_subproblem(&mut self, subproblem: SubProblem) {
        let ctx = &self.local_problem_ctx.clone();

        let mut solver = DPLLSolver::with_decisions(&ctx.problem, &subproblem.initial_decision);
        self.strat.on_new_subproblem(&solver);

        let mut falsified_lit = solver.make_branching_decision();
        self.strat.after_decision(&solver);

        loop {
            // Check if another worker has already found a solution
            if ctx.solution_found_flag.load(atomic::Ordering::Acquire) {
                break;
            }

            match solver.step(falsified_lit) {
                SolverAction::SAT => {
                    // Signal other workers to stop working on this problem
                    ctx.solution_found_flag
                        .store(true, atomic::Ordering::Release);

                    // Send the found solution
                    let _ = ctx.solution_sender.send(solver.assignment.to_solution());
                    break;
                }
                SolverAction::Decision(next_falsified_lit) => {
                    falsified_lit = next_falsified_lit;
                    self.strat.after_decision(&solver);
                }
                SolverAction::Backtrack => {
                    if_metrics!(
                        let mut path = Vec::new();
                        solver.assignment.extract_decisions(&mut path);
                        metrics::record_path(&path);
                        metrics::record_conflict(self._id);
                    );

                    if let Some(new_falsified_lit) = self.backtrack(&mut solver) {
                        falsified_lit = new_falsified_lit;
                        continue;
                    }

                    let is_last_active = self
                        .num_active_workers
                        .fetch_sub(1, atomic::Ordering::AcqRel)
                        == 1;

                    if !is_last_active
                        && let Some((lit, new_solver)) = self.strat.find_new_work(
                            solver,
                            &ctx.problem,
                            &ctx.solution_found_flag,
                            &self.num_active_workers,
                        )
                    {
                        falsified_lit = lit;
                        solver = new_solver;
                        self.num_active_workers
                            .fetch_add(1, atomic::Ordering::Release);
                        self.strat.on_new_subproblem(&solver);

                        continue;
                    }

                    // run() expects us to be active when we return
                    self.num_active_workers
                        .fetch_add(1, atomic::Ordering::Release);
                    break;
                }
            }
        }
    }

    pub fn backtrack(&mut self, solver: &mut DPLLSolver) -> Option<Lit> {
        while !self.strat.should_stop_backtracking_early(solver) {
            // Try to backtrack one level
            match solver.backtrack_one_level() {
                BacktrackResult::TryAlternative(new_falsified_lit) => {
                    return Some(new_falsified_lit); // Success, try this alternative path
                }
                BacktrackResult::NoMoreDecisions => {
                    return None;
                }
                BacktrackResult::Continue => {
                    continue;
                }
            }
        }

        None
    }

    pub fn sync_problem_context(&mut self, expected_pid: usize) {
        let guard = match self.shared_ctx.read() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.current_pid != expected_pid {
            eprintln!("Worker received subproblem with stale PID.");
            return;
        }

        self.local_problem_ctx = guard.problem_ctx.clone();
        self.cached_pid = guard.current_pid;

        self.strat.on_new_problem(&self.local_problem_ctx.problem);
    }
}
