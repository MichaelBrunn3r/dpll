use crate::{
    clause::Lit,
    dpll::DPLLSolver,
    generator, if_metrics,
    pool::WorkerPoolStrategy,
    problem::Problem,
    utils::{Backoff, opt_bool::OptBool},
    worker::{core::WorkerCore, metrics::MetricsLogger, stealing::StealingWorker},
};
use itertools::Itertools;
use log::{error, info};
use std::{
    num::{NonZero, NonZeroUsize},
    sync::{
        Arc, RwLock,
        atomic::{self, AtomicBool, AtomicUsize},
        mpsc,
    },
    thread::{self},
    time::Duration,
    vec,
};

pub struct ThreadedWorkerPool {
    job_sender: crossbeam_channel::Sender<SubProblem>,
    _workers: Vec<thread::JoinHandle<()>>,
    shared_ctx: Arc<RwLock<SharedContext>>,
    pub num_workers: NonZero<usize>,
    pub active_workers: Arc<AtomicUsize>,
}

impl ThreadedWorkerPool {
    pub fn new(num_workers: NonZero<usize>, steal: bool) -> Self {
        // Create communication utilities for subproblem distribution
        let (subproblem_sender, subproblem_receiver) = crossbeam_channel::unbounded::<SubProblem>();
        let shared_ctx = Arc::new(RwLock::new(SharedContext {
            current_pid: 0,
            problem_ctx: Arc::new(ProblemContext::default()),
        }));

        // Spawn worker threads
        let mut workers = Vec::with_capacity(num_workers.get());
        let num_active_workers = Arc::new(AtomicUsize::new(0));
        if steal {
            Self::start_stealing_workers(
                &mut workers,
                subproblem_receiver,
                &num_active_workers,
                &shared_ctx,
            );
        } else {
            Self::start_basic_workers(
                &mut workers,
                subproblem_receiver,
                &num_active_workers,
                &shared_ctx,
            );
        }

        info!("Initialized pool with {} worker thread(s).", num_workers);
        Self {
            job_sender: subproblem_sender,
            _workers: workers,
            shared_ctx,
            num_workers,
            active_workers: num_active_workers,
        }
    }

    pub fn start_stealing_workers(
        workers: &mut Vec<thread::JoinHandle<()>>,
        subproblem_receiver: crossbeam_channel::Receiver<SubProblem>,
        num_active_workers: &Arc<AtomicUsize>,
        shared_ctx: &Arc<RwLock<SharedContext>>,
    ) {
        let num_workers = workers.capacity();

        // Create local queues and stealers from those queues
        let mut local_queues = Vec::with_capacity(num_workers);
        let mut all_stealers = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let local_queue = crossbeam_deque::Worker::new_lifo();
            all_stealers.push(local_queue.stealer());
            local_queues.push(local_queue);
        }

        for worker_id in 0..num_workers {
            let local_queue = local_queues.remove(0);
            let ctx = shared_ctx.clone();
            let num_active_workers = num_active_workers.clone();
            let subproblem_receiver = subproblem_receiver.clone();

            // Give worker stealers for all OTHER workers' queues
            let peer_queues = all_stealers
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != worker_id)
                .map(|(peer_id, stealer)| (stealer.clone(), peer_id))
                .collect();

            workers.push(thread::spawn(move || {
                let behavior = StealingWorker::new(worker_id, local_queue, peer_queues);
                WorkerCore::new(
                    worker_id,
                    num_active_workers,
                    subproblem_receiver.clone(),
                    ctx,
                    behavior,
                )
                .run();
            }));
        }
    }

    pub fn start_basic_workers(
        workers: &mut Vec<thread::JoinHandle<()>>,
        subproblem_receiver: crossbeam_channel::Receiver<SubProblem>,
        num_active_workers: &Arc<AtomicUsize>,
        shared_ctx: &Arc<RwLock<SharedContext>>,
    ) {
        let num_workers = workers.capacity();

        for worker_id in 0..num_workers {
            let rx: crossbeam_channel::Receiver<SubProblem> = subproblem_receiver.clone();
            let ctx = shared_ctx.clone();
            let active = num_active_workers.clone();

            workers.push(thread::spawn(move || {
                let behavior = crate::worker::BasicWorker {};
                WorkerCore::new(worker_id, active, rx, ctx, behavior).run();
            }));
        }
    }

    pub fn await_result(&self, solution_receiver: &mpsc::Receiver<Vec<bool>>) -> Option<Vec<bool>> {
        let mut backoff = Backoff::new(
            128,
            512,
            Duration::from_micros(1),
            Duration::from_millis(10),
            1.1,
        );
        let mut logger = MetricsLogger::new("metrics.bin", Duration::from_millis(100))
            .expect("Failed to initialize logger");

        let result = loop {
            // Check if we have received a solution => SAT
            if let Ok(solution) = solution_receiver.try_recv() {
                break Some(solution);
            }

            // Check if all workers are idle => UNSAT
            let num_active = self.active_workers.load(atomic::Ordering::Acquire);
            let pending_subproblems = self.job_sender.len();
            if num_active == 0 && pending_subproblems == 0 {
                break None;
            }

            logger.tick();
            backoff.wait();
        };

        if_metrics! {
            match logger.close() {
                Ok(filename) => info!("Saved captured metrics to '{}'", filename),
                Err(e) => error!("Failed to close metrics logger: {}", e),
            }
        }

        result
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
    fn generate_combinations<'p>(
        problem: &'p Problem,
        split_vars: &'p Vec<usize>,
    ) -> impl Iterator<Item = DecisionPath> + 'p {
        let clauses_containing_split_vars = split_vars
            .iter()
            .flat_map(|&var| problem.clauses_containing_var(var))
            .unique_by(|c| *c as *const _)
            .collect::<Vec<_>>();

        let combinations = 1usize << split_vars.len();

        generator!(move || {
            for combination in 0..combinations {
                let mut decisions = DecisionPath(Vec::with_capacity(split_vars.len()));

                for (bit_idx, &var) in split_vars.iter().enumerate() {
                    let val = (combination & (1 << bit_idx)) != 0;
                    decisions.0.push(Lit::new(var, val));
                }

                // Check if any clause containing split vars is unsatisfied
                if clauses_containing_split_vars
                    .iter()
                    .any(|clause| clause.is_unsatisfied_by_decisions(&decisions))
                {
                    continue; // Skip this assignment as it leads to unsatisfied clauses
                }

                yield decisions;
            }
        })
    }

    /// Calculates the optimal number of splits based on the problem size and number of workers.
    /// Returns None if the problem is too small to benefit from parallelism.
    pub fn calculate_optimal_splits(problem: &Problem, num_workers: NonZeroUsize) -> Option<usize> {
        let num_splits = (num_workers.get() as f64).log2().ceil() as usize;
        if num_splits > problem.num_vars {
            None
        } else {
            Some(num_splits)
        }
    }
}

impl WorkerPoolStrategy for ThreadedWorkerPool {
    fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        let num_splits = match Self::calculate_optimal_splits(&problem, self.num_workers) {
            Some(n) => n,
            None => {
                // Problem too small to benefit from parallelism => solve directly
                return DPLLSolver::with_decisions(&problem, &DecisionPath(Vec::new())).solve();
            }
        };

        // Notify all workers of the new problem
        let solution_found = Arc::new(AtomicBool::new(false));
        let (solution_sender, solution_receiver) = mpsc::channel();

        let current_pid = {
            let mut ctx_lock = self.shared_ctx.write().unwrap();
            ctx_lock.current_pid += 1;

            ctx_lock.problem_ctx = Arc::new(ProblemContext {
                problem: Arc::clone(&problem),
                solution_found_flag: Arc::clone(&solution_found),
                solution_sender: solution_sender.clone(),
            });
            ctx_lock.current_pid
        };

        let split_vars = Arc::new(Self::select_split_vars(&problem, num_splits));

        for initial_decisions in Self::generate_combinations(&problem, &split_vars) {
            // Check if a solution has been found while generating jobs
            if solution_found.load(atomic::Ordering::Acquire) {
                break;
            }

            if self
                .job_sender
                .send(SubProblem::new(current_pid, initial_decisions))
                .is_err()
            {
                return None;
            }
        }

        drop(solution_sender);

        self.await_result(&solution_receiver)
    }
}

pub struct SharedContext {
    pub current_pid: usize,
    pub problem_ctx: Arc<ProblemContext>,
}

pub struct ProblemContext {
    /// The problem to solve.
    pub problem: Arc<Problem>,
    /// Flag indicating if a solution has been found.
    pub solution_found_flag: Arc<AtomicBool>,
    pub solution_sender: mpsc::Sender<Vec<bool>>,
}

impl Default for ProblemContext {
    fn default() -> Self {
        Self {
            problem: Arc::new(Problem::default()),
            solution_found_flag: Arc::new(AtomicBool::new(false)),
            solution_sender: mpsc::channel().0,
        }
    }
}

pub struct SubProblem {
    /// The ID of the associated problem.
    pub pid: usize,
    /// The initial assignment for the sub-problem.
    pub initial_decision: DecisionPath,
}

impl SubProblem {
    pub fn new(pid: usize, initial_decision: DecisionPath) -> Self {
        Self {
            pid,
            initial_decision,
        }
    }
}

/// A sequence of variable assignment decisions made during search.
/// Can be stolen by idle workers and helps them reconstruct search states.
#[derive(Debug)]
pub struct DecisionPath(pub Vec<Lit>);

impl DecisionPath {
    pub fn to_assignment(&self, num_vars: usize) -> Vec<OptBool> {
        let mut assignment = vec![OptBool::Unassigned; num_vars];
        for lit in &self.0 {
            assignment[lit.var() as usize] = OptBool::from(lit.is_pos());
        }
        assignment
    }
}

impl From<Vec<Lit>> for DecisionPath {
    fn from(decisions: Vec<Lit>) -> Self {
        Self(decisions)
    }
}

impl From<Vec<i32>> for DecisionPath {
    fn from(value: Vec<i32>) -> Self {
        DecisionPath(value.iter().map(|&x| Lit::from(x)).collect())
    }
}
