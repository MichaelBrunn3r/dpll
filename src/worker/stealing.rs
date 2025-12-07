use std::{
    sync::atomic::{self},
    time::Duration,
};

use crate::{
    clause::Lit, dpll::DPLLSolver, pool::DecisionPath, problem::Problem, stats,
    utils::opt_bool::OptBool, worker::WorkerStrategy,
};

pub struct StealingWorker {
    _id: usize,
    /// Local queue of offered decision paths.
    local_queue: crossbeam_deque::Worker<DecisionPath>,
    /// Queues of peer workers to steal from, along with their IDs.
    peer_queues: Vec<(crossbeam_deque::Stealer<DecisionPath>, usize)>,
    /// The decision level up to which to offer alternative paths to peers.
    offer_threshold: usize,
    /// Maximum number of decision levels to keep in the local queue.
    queue_limit: usize,
    /// The deepest decision level that has been offered to peers.
    deepest_offered_level: usize,
}

impl StealingWorker {
    pub fn new(
        id: usize,
        local_queue: crossbeam_deque::Worker<DecisionPath>,
        peer_queues: Vec<(crossbeam_deque::Stealer<DecisionPath>, usize)>,
    ) -> Self {
        StealingWorker {
            _id: id,
            local_queue,
            peer_queues,
            offer_threshold: 0,
            queue_limit: 15,
            deepest_offered_level: 0,
        }
    }

    pub fn try_steal_from_peers(&mut self) -> Option<DecisionPath> {
        for (stealer, _peer_id) in &self.peer_queues {
            match stealer.steal() {
                crossbeam_deque::Steal::Success(path) => {
                    stats!(self._id, |worker, peers| {
                        worker.steal.fetch_add(1, atomic::Ordering::Relaxed);
                        peers[*_peer_id]
                            .stolen_from
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    });
                    return Some(path);
                }
                _ => continue,
            }
        }
        None
    }
}

impl WorkerStrategy for StealingWorker {
    #[inline(always)]
    fn on_new_problem(&mut self, problem: &Problem) {
        self.offer_threshold = 15;
        self.deepest_offered_level = 0;
    }

    #[inline(always)]
    fn after_decision(&mut self, solver: &DPLLSolver) {
        if solver.assignment.last_decision() != OptBool::True {
            return;
        }

        let level = solver.assignment.decision_level();

        let current_q_len = self.local_queue.len();
        stats!(self._id, |worker, peers| {
            worker
                .queue_len
                .store(current_q_len as u64, atomic::Ordering::Relaxed);
        });

        // Only offer if we're past the threshold AND the queue has space.
        if level > self.offer_threshold || current_q_len >= self.queue_limit {
            return;
        }

        let mut decisions = solver.assignment.extract_decision();

        // Get the last decision made
        let (_, last_val) = if let Some(last_decision) = decisions.last_mut() {
            last_decision
        } else {
            // No decisions made yet
            return;
        };

        *last_val = false; // Try alternative path (i.e. false)

        stats!(self._id, |worker, peers| {
            worker.push.fetch_add(1, atomic::Ordering::Relaxed);
        });
        self.local_queue.push(DecisionPath::from(decisions));
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
        stats!(self._id, |worker, peers| {
            debug_assert!(
                self.local_queue.is_empty(),
                "Expected empty local queue when stealing work, but there are {} paths available.",
                self.local_queue.len()
            );
            worker.queue_len.store(0, atomic::Ordering::Relaxed);
        });

        #[cfg(feature = "stats")]
        let mut prev_awake = std::time::Instant::now();

        let result = loop {
            if let Some(stolen_path) = self.try_steal_from_peers() {
                let mut init_assignment = vec![OptBool::Unassigned; problem.num_vars];
                for (var, val) in &stolen_path.decisions {
                    init_assignment[*var] = OptBool::from(*val);
                }

                let solver = DPLLSolver::with_assignment(
                    &problem,
                    init_assignment,
                    stolen_path.decisions.len(),
                );
                let (last_var, last_val) = stolen_path.decisions.last().unwrap();
                let falsified_lit = Lit::new(*last_var, *last_val).negated();

                break Some((falsified_lit, solver));
            }

            if solution_found_flag.load(atomic::Ordering::Acquire)
                || num_active_workers.load(atomic::Ordering::Acquire) == 0
            {
                break None;
            }

            stats!(self._id, |worker, peers| {
                worker.idle_micros.fetch_add(
                    prev_awake.elapsed().as_micros() as u64,
                    atomic::Ordering::Relaxed,
                );
                prev_awake = std::time::Instant::now();
            });
            std::thread::sleep(Duration::from_micros(100));
        };

        stats!(self._id, |worker, peers| {
            worker.idle_micros.fetch_add(
                prev_awake.elapsed().as_micros() as u64,
                atomic::Ordering::Relaxed,
            );
        });

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
            stats!(self._id, |worker, peers| {
                worker.pop.fetch_add(1, atomic::Ordering::Relaxed);
            });
            self.deepest_offered_level = path.decisions.len() - 1;
            return false;
        }

        true
    }
}
