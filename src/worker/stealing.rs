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
    local_queue: crossbeam_deque::Worker<DecisionPath>,
    peer_queues: Vec<(crossbeam_deque::Stealer<DecisionPath>, usize)>,
    offer_threshold: usize,
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
        self.offer_threshold = 10;
    }

    #[inline(always)]
    fn after_decision(&mut self, solver: &DPLLSolver) {
        let level = solver.assignment.decision_level();
        if level > self.offer_threshold {
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
    }

    #[inline(always)]
    fn find_new_work<'p>(
        &mut self,
        _current_solver: DPLLSolver,
        problem: &'p Problem,
        solution_found_flag: &atomic::AtomicBool,
        num_active_workers: &atomic::AtomicUsize,
    ) -> Option<(Lit, DPLLSolver<'p>)> {
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

        // Check if the alternative path we offered at this level was stolen.
        // We only offer paths up to offer_threshold, so only check in that range.
        let current_level = solver.assignment.decision_level();
        if current_level <= self.offer_threshold {
            stats!(self._id, |worker, peers| {
                worker.pop.fetch_add(1, atomic::Ordering::Relaxed);
            });
            if let Some(_path) = self.local_queue.pop() {
                return false;
            }

            return true;
        }
        false
    }
}
