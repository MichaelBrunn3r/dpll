use std::{
    sync::atomic::{self},
    time::Duration,
};

use crate::{
    clause::Lit, dpll::DPLLSolver, pool::DecisionPath, problem::Problem, utils::opt_bool::OptBool,
    worker::WorkerStrategy,
};

pub struct StealingWorker {
    local_queue: crossbeam_deque::Worker<DecisionPath>,
    peer_queues: Vec<crossbeam_deque::Stealer<DecisionPath>>,
    offer_threshold: usize,
}

impl StealingWorker {
    pub fn new(
        local_queue: crossbeam_deque::Worker<DecisionPath>,
        peer_queues: Vec<crossbeam_deque::Stealer<DecisionPath>>,
    ) -> Self {
        StealingWorker {
            local_queue,
            peer_queues,
            offer_threshold: 0,
        }
    }

    pub fn try_steal_from_peers(&mut self) -> Option<DecisionPath> {
        for stealer in &self.peer_queues {
            match stealer.steal() {
                crossbeam_deque::Steal::Success(path) => {
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
        self.offer_threshold = (problem.num_vars as f64).log2().ceil() as usize;
    }

    #[inline(always)]
    fn after_decision(&self, solver: &DPLLSolver) {
        if solver.assignment.decision_level() > self.offer_threshold {
            return;
        }

        let mut decisions = solver.assignment.extract_decision();

        // info!(
        //     "[{}] Offer @{} #{}",
        //     self._id,
        //     decisions.len(),
        //     self.level_offered_counts[decisions.len()]
        // );

        // Get the last decision made
        let (_, last_val) = if let Some(last_decision) = decisions.last_mut() {
            last_decision
        } else {
            // No decisions made yet
            return;
        };

        *last_val = false;

        self.local_queue.push(DecisionPath::from(decisions));
    }

    #[inline(always)]
    fn find_new_work<'p>(
        &mut self,
        _current_solver: DPLLSolver,
        problem: &'p Problem,
        solution_found_flag: &atomic::AtomicBool,
    ) -> Option<(Lit, DPLLSolver<'p>)> {
        loop {
            // Try to steal a decision path from peers
            match self.try_steal_from_peers() {
                Some(stolen_path) => {
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

                    // info!(
                    //     "[{}] Stole@{} => continue @{}",
                    //     self._id,
                    //     stolen_path.decisions.len(),
                    //     solver.assignment.decision_level()
                    // );
                    return Some((falsified_lit, solver));
                }
                None => {
                    if solution_found_flag.load(atomic::Ordering::Relaxed) {
                        return None;
                    }
                    std::thread::sleep(Duration::from_micros(100));
                }
            }
        }
    }

    #[inline(always)]
    fn should_stop_backtracking_early(&self, solver: &DPLLSolver) -> bool {
        // Check if the alternative path we offered at this level was stolen.
        // We only offer paths up to offer_threshold, so only check in that range.
        if solver.assignment.decision_level() <= self.offer_threshold
            && self.local_queue.pop().is_none()
        {
            // The queue is empty => the alternative path was stolen
            return true;
        }
        false
    }
}
