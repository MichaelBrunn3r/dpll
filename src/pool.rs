use crate::{
    clause::VariableId, dpll::DPLLSolver, generator, problem::Problem, utils::opt_bool::OptBool,
    worker::Worker,
};
use itertools::Itertools;
use std::{
    sync::{
        Arc, RwLock,
        atomic::{self, AtomicBool},
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
}

impl WorkerPool {
    pub fn new(num_workers: usize) -> Self {
        // Limit number of workers to available parallelism
        let num_workers = num_workers.min(available_parallelism().map(|n| n.get()).unwrap_or(1));

        // Single-threaded mode
        if num_workers <= 1 {
            return Self {
                job_sender: None,
                _workers: Vec::new(),
                shared_ctx: Arc::new(RwLock::new(SharedContext {
                    current_pid: 0,
                    problem_ctx: Arc::new(ProblemContext::default()),
                })),
                num_workers: 1,
            };
        }

        // Setup coordination utilities
        let (task_sender, task_receiver) = crossbeam_channel::unbounded::<SubProblem>();
        let shared_ctx = Arc::new(RwLock::new(SharedContext {
            current_pid: 0,
            problem_ctx: Arc::new(ProblemContext::default()),
        }));

        // Create local queues and stealers from those queues
        let mut local_queues = Vec::with_capacity(num_workers);
        let mut all_stealers = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let local_queue = crossbeam_deque::Worker::new_lifo();
            all_stealers.push(local_queue.stealer());
            local_queues.push(local_queue);
        }

        // Spawn worker threads
        let mut workers = Vec::with_capacity(num_workers);
        for worker_id in 0..num_workers {
            let rx = task_receiver.clone();
            let ctx = shared_ctx.clone();
            let local_queue = local_queues.remove(0);

            // Give worker stealers for all OTHER workers' queues
            let peer_queues = all_stealers
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != worker_id)
                .map(|(_, stealer)| stealer.clone())
                .collect();

            workers.push(thread::spawn(move || {
                Worker::new(worker_id, rx, ctx, local_queue, peer_queues).run();
            }));
        }

        Self {
            job_sender: Some(task_sender),
            _workers: workers,
            shared_ctx,
            num_workers,
        }
    }

    pub fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        let job_sender = match &self.job_sender {
            Some(tx) => tx,
            None => {
                // Single-threaded mode
                let mut assignment_buffer = vec![OptBool::Unassigned; problem.num_vars];
                return DPLLSolver::with_assignment(&problem, &mut assignment_buffer, 0).solve();
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
                sender: tx.clone(),
            });
            ctx_lock.current_pid
        };

        let depth = Self::calculate_depth(self.num_workers, problem.num_vars);
        let split_vars = Arc::new(Self::select_split_vars(&problem, depth));

        let mut active_jobs = 0;
        for init_assignment in Self::generate_combinations(&problem, &split_vars) {
            // Check if a solution has been found while generating jobs
            if solution_found.load(atomic::Ordering::Relaxed) {
                break;
            }

            if job_sender
                .send(SubProblem::new(current_pid, init_assignment))
                .is_err()
            {
                return None;
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
                WorkerResult::SAT(solution) => return Some(solution),
                WorkerResult::UNSAT => {
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
    /// Channel to send back job results.
    pub sender: mpsc::Sender<WorkerResult>,
}

impl Default for ProblemContext {
    fn default() -> Self {
        Self {
            problem: Arc::new(Problem::default()),
            solution_found_flag: Arc::new(AtomicBool::new(false)),
            sender: mpsc::channel().0,
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

/// Result sent back from workers when they complete a sub-problem.
pub enum WorkerResult {
    /// The sub-problem is satisfiable, with the given solution.
    SAT(Vec<bool>),
    /// The sub-problem is unsatisfiable.
    UNSAT,
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
