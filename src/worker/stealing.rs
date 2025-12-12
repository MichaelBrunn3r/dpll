use std::{
    sync::{
        Arc,
        atomic::{self},
    },
    time::Duration,
};

use crate::{
    clause::Lit,
    dpll::DPLLSolver,
    if_metrics,
    problem::Problem,
    utils::{VecExt, opt_bool::OptBool},
    worker::{WorkerStrategy, metrics},
};

pub struct StealingWorker {
    _id: usize,
    /// Local queue of offered decision paths.
    local_queue: crossbeam_deque::Worker<Arc<DecisionPath>>,
    /// Queues of peer workers to steal from, along with their IDs.
    peer_queues: Vec<(crossbeam_deque::Stealer<Arc<DecisionPath>>, usize)>,
    /// The decision level up to which to offer alternative paths to peers.
    offer_threshold: usize,
    /// Maximum number of decision levels to keep in the local queue.
    queue_limit: usize,
    /// The deepest decision level that has been offered to peers.
    deepest_offered_level: usize,
    /// Path pool
    path_pool: Vec<Arc<DecisionPath>>,
}

impl StealingWorker {
    pub fn new(
        id: usize,
        local_queue: crossbeam_deque::Worker<Arc<DecisionPath>>,
        peer_queues: Vec<(crossbeam_deque::Stealer<Arc<DecisionPath>>, usize)>,
    ) -> Self {
        let queue_limit = 5;

        StealingWorker {
            _id: id,
            local_queue,
            peer_queues,
            offer_threshold: 0,
            queue_limit,
            deepest_offered_level: 0,
            path_pool: Vec::with_capacity(queue_limit),
        }
    }

    pub fn try_steal_from_peers(&mut self) -> Option<Arc<DecisionPath>> {
        for (stealer, peer_id) in &self.peer_queues {
            match stealer.steal() {
                crossbeam_deque::Steal::Success(path) => {
                    metrics::record_stole_from(self._id, *peer_id);
                    return Some(path);
                }
                _ => metrics::record_failed_to_steal(self._id),
            }
        }
        None
    }

    fn get_or_create_decision_path(&mut self) -> Arc<DecisionPath> {
        self.path_pool.pop().unwrap_or_else(|| {
            metrics::record_allocated_path();
            Arc::new(DecisionPath(Vec::with_capacity(self.offer_threshold + 1)))
        })
    }
}

impl WorkerStrategy for StealingWorker {
    #[inline(always)]
    fn on_new_problem(&mut self, problem: &Problem) {
        self.offer_threshold = 15.min(problem.num_vars / 2);
        self.deepest_offered_level = 0;

        // Clear local queue into the path pool
        while let Some(path) = self.local_queue.pop() {
            if self.path_pool.len() < self.queue_limit {
                self.path_pool.push(path);
            }
        }

        // Ensure all paths in the pool have enough capacity
        for path in self.path_pool.iter_mut() {
            if let Some(path) = Arc::get_mut(path) {
                path.0.ensure_capacity(problem.num_vars);
            }
        }

        // Fill the path pool up to the queue limit
        while self.path_pool.len() < self.queue_limit {
            let decisions = Vec::with_capacity(problem.num_vars);
            self.path_pool.push(Arc::new(DecisionPath(decisions)));
        }
    }

    #[inline(always)]
    fn after_decision(&mut self, solver: &DPLLSolver) {
        if solver.assignment.last_decision() != OptBool::True {
            return;
        }

        let level = solver.assignment.decision_level();
        let current_q_len = self.local_queue.len();
        metrics::record_queue_length(self._id, current_q_len as u64);

        if level > self.offer_threshold {
            metrics::record_path_exceeds_offer_threshold(self._id);
            return;
        }

        // Only offer if we're past the threshold AND the queue has space.
        if current_q_len >= self.queue_limit {
            metrics::record_queue_full(self._id);
            return;
        }

        let mut path_ref = self.get_or_create_decision_path();
        let mut path = &mut Arc::get_mut(&mut path_ref)
            .expect("Invariant violated: Pooled Arc has multiple owners")
            .0;
        path.clear();
        solver.assignment.extract_decisions_into(&mut path);

        // Get the last decision made
        let last_lit = if let Some(last_decision) = path.last_mut() {
            last_decision
        } else {
            // No decisions made yet
            self.path_pool.push(path_ref); // Return to pool
            return;
        };

        *last_lit = last_lit.negated(); // Try alternative path (i.e. false)

        metrics::record_push_into_queue(self._id);
        self.local_queue.push(path_ref);
        self.deepest_offered_level = level;
    }

    #[inline(always)]
    fn find_new_work<'p>(
        &mut self,
        _current_solver: DPLLSolver,
        problem: &'p Problem,
        solution_found_flag: &atomic::AtomicBool,
        num_active_workers: &atomic::AtomicUsize,
    ) -> Option<(Lit, DPLLSolver<'p>)> {
        debug_assert!(
            self.local_queue.is_empty(),
            "Expected empty local queue when stealing work, but there are {} paths available.",
            self.local_queue.len()
        );
        if_metrics! { let mut prev_awake = std::time::Instant::now();};

        let result = loop {
            if let Some(stolen_path) = self.try_steal_from_peers() {
                let mut init_assignment = vec![OptBool::Unassigned; problem.num_vars];
                for lit in &stolen_path.0 {
                    init_assignment[lit.var() as usize] = OptBool::from(lit.is_pos());
                }

                let solver =
                    DPLLSolver::with_assignment(&problem, init_assignment, stolen_path.0.len());
                let last_lit = stolen_path.0.last().unwrap();
                let falsified_lit = last_lit.inverted();

                self.path_pool.push(stolen_path);
                break Some((falsified_lit, solver));
            }

            if solution_found_flag.load(atomic::Ordering::Acquire)
                || num_active_workers.load(atomic::Ordering::Acquire) == 0
            {
                break None;
            }

            if_metrics! {
                metrics::record_idle_for(self._id, prev_awake.elapsed().as_micros() as u64);
                prev_awake = std::time::Instant::now();
            };
            std::thread::sleep(Duration::from_micros(100));
        };

        if_metrics! { metrics::record_idle_for(self._id, prev_awake.elapsed().as_micros() as u64);}
        result
    }

    #[inline(always)]
    fn should_stop_backtracking_early(&mut self, solver: &DPLLSolver) -> bool {
        // If the last decision was a true branch, we didn't push it into the local queue.
        // => Could not have been stolen.
        if solver.assignment.last_decision() != OptBool::True {
            return false;
        }

        let current_level = solver.assignment.decision_level();

        // Only check levels we actually offered.
        if current_level > self.offer_threshold || current_level > self.deepest_offered_level {
            return false;
        }

        // Check if the alternative path we offered at this level was stolen.
        // We only offer paths up to offer_threshold, so only check in that range.
        if let Some(path) = self.local_queue.pop() {
            metrics::record_pop_from_queue(self._id);

            self.deepest_offered_level = path.0.len() - 1;
            self.path_pool.push(path);
            return false;
        }

        metrics::record_work_was_stolen(self._id);
        true
    }
}

/// A sequence of variable assignment decisions made during search.
/// Can be stolen by idle workers and helps them reconstruct search states.
#[derive(Debug)]
pub struct DecisionPath(pub Vec<Lit>);

impl From<Vec<Lit>> for DecisionPath {
    fn from(decisions: Vec<Lit>) -> Self {
        Self(decisions)
    }
}
