use crate::{
    clause::VariableId,
    dpll::DPLLSolver,
    generator,
    problem::Problem,
    utils::opt_bool::OptBool,
    worker::{WorkerStrategyType, core::WorkerCore, stealing::StealingWorker},
};
use itertools::Itertools;
use std::{
    sync::{
        Arc, RwLock,
        atomic::{self, AtomicBool, AtomicUsize},
        mpsc,
    },
    thread::{self, available_parallelism},
    vec,
};

pub struct WorkerPool {
    job_sender: Option<crossbeam_channel::Sender<SubProblem>>,
    _workers: Vec<thread::JoinHandle<()>>,
    shared_ctx: Arc<RwLock<SharedContext>>,
    pub num_workers: usize,
    pub active_workers: Arc<AtomicUsize>,
}

impl WorkerPool {
    pub fn new(num_workers: usize, strategy: WorkerStrategyType) -> Self {
        // Limit number of workers to available parallelism
        let num_workers = num_workers.min(available_parallelism().map(|n| n.get()).unwrap_or(1));
        if num_workers <= 1 {
            return Self::new_single_threaded();
        }

        // Create communication utilities for subproblem distribution
        let (subproblem_sender, subproblem_receiver) = crossbeam_channel::unbounded::<SubProblem>();
        let shared_ctx = Arc::new(RwLock::new(SharedContext {
            current_pid: 0,
            problem_ctx: Arc::new(ProblemContext::default()),
        }));

        // Spawn worker threads
        let mut workers = Vec::with_capacity(num_workers);
        let num_active_workers = Arc::new(AtomicUsize::new(0));
        match strategy {
            WorkerStrategyType::Basic => {
                Self::start_basic_workers(
                    &mut workers,
                    subproblem_receiver,
                    &num_active_workers,
                    &shared_ctx,
                );
            }
            WorkerStrategyType::Stealing => {
                Self::start_stealing_workers(
                    &mut workers,
                    subproblem_receiver,
                    &num_active_workers,
                    &shared_ctx,
                );
            }
        }

        Self {
            job_sender: Some(subproblem_sender),
            _workers: workers,
            shared_ctx,
            num_workers,
            active_workers: num_active_workers,
        }
    }

    pub fn new_single_threaded() -> Self {
        Self {
            job_sender: None,
            _workers: Vec::new(),
            shared_ctx: Arc::new(RwLock::new(SharedContext {
                current_pid: 0,
                problem_ctx: Arc::new(ProblemContext::default()),
            })),
            num_workers: 1,
            active_workers: Arc::new(AtomicUsize::new(0)),
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
                .map(|(_, stealer)| stealer.clone())
                .collect();

            workers.push(thread::spawn(move || {
                let behavior = StealingWorker::new(local_queue, peer_queues);
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

    pub fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        let job_sender = match &self.job_sender {
            Some(tx) => tx,
            None => {
                // Single-threaded mode
                let assignment_buffer = vec![OptBool::Unassigned; problem.num_vars];
                return DPLLSolver::with_assignment(&problem, assignment_buffer, 0).solve();
            }
        };

        // Notify all workers of the new problem
        let solution_found = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let current_pid = {
            let mut ctx_lock = self.shared_ctx.write().unwrap();
            ctx_lock.current_pid += 1;

            ctx_lock.problem_ctx = Arc::new(ProblemContext {
                problem: Arc::clone(&problem),
                solution_found_flag: Arc::clone(&solution_found),
                solution_sender: tx.clone(),
            });
            ctx_lock.current_pid
        };

        let depth = Self::calculate_depth(self.num_workers, problem.num_vars);
        let split_vars = Arc::new(Self::select_split_vars(&problem, depth));

        self.active_workers
            .store(self.num_workers, atomic::Ordering::Release);
        for init_assignment in Self::generate_combinations(&problem, &split_vars) {
            // Check if a solution has been found while generating jobs
            if solution_found.load(atomic::Ordering::Acquire) {
                break;
            }

            if job_sender
                .send(SubProblem::new(current_pid, init_assignment))
                .is_err()
            {
                return None;
            }
        }

        drop(tx);

        loop {
            // Check if we have received a solution
            if let Ok(solution) = rx.try_recv() {
                return Some(solution);
            }

            let num_idle = self.active_workers.load(atomic::Ordering::Acquire);
            let pending_subproblems = job_sender.len();

            if num_idle == self.num_workers && pending_subproblems == 0 {
                return None; // UNSAT
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
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
    fn generate_combinations<'p>(
        problem: &'p Problem,
        split_vars: &'p Vec<usize>,
    ) -> impl Iterator<Item = Vec<OptBool>> + 'p {
        let clauses_containing_split_vars = split_vars
            .iter()
            .flat_map(|&var| problem.clauses_containing_var(var))
            .unique_by(|c| *c as *const _)
            .collect::<Vec<_>>();

        let num_vars = problem.num_vars;
        let combinations = 1usize << split_vars.len();

        generator!(move || {
            for combination in 0..combinations {
                let mut assignment = vec![OptBool::Unassigned; num_vars];

                for (bit_idx, &var) in split_vars.iter().enumerate() {
                    let val = (combination & (1 << bit_idx)) != 0;
                    assignment[var] = OptBool::from(val);
                }

                // Check if any clause containing split vars is unsatisfied
                if clauses_containing_split_vars
                    .iter()
                    .any(|clause| clause.is_unsatisfied_by_partial(&assignment))
                {
                    continue; // Skip this assignment as it leads to unsatisfied clauses
                }

                yield assignment
            }
        })
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
    pub init_assignment: Vec<OptBool>,
}

impl SubProblem {
    pub fn new(pid: usize, init_assignment: Vec<OptBool>) -> Self {
        Self {
            pid,
            init_assignment,
        }
    }
}

/// A sequence of variable assignment decisions made during search.
/// Can be stolen by idle workers and helps them reconstruct search states.
pub struct DecisionPath {
    /// The sequence of variable assignments (variable ID and assigned value).
    pub decisions: Vec<(VariableId, bool)>,
}

impl From<Vec<(VariableId, bool)>> for DecisionPath {
    fn from(decisions: Vec<(VariableId, bool)>) -> Self {
        Self { decisions }
    }
}
