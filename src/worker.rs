use log::{error, info};

use crate::{
    clause::{Lit, VariableId},
    dpll::{DPLLSolver, SolverAction},
    partial_assignment::BacktrackResult,
    pool::{DecisionPath, ProblemContext, SharedContext, SubProblem, WorkerResult},
    utils::opt_bool::OptBool,
};
use std::sync::{Arc, RwLock, atomic};

pub struct Worker {
    _id: usize,
    subproblem_receiver: crossbeam_channel::Receiver<SubProblem>,

    // --- Context ---
    shared_ctx: Arc<RwLock<SharedContext>>,
    local_pid: usize,
    local_problem_ctx: Arc<ProblemContext>,

    // --- Work-stealing ---
    local_queue: crossbeam_deque::Worker<DecisionPath>,
    peer_queues: Vec<crossbeam_deque::Stealer<DecisionPath>>,
    offer_threshold: usize,

    // --- debug ---
    level_offered_counts: Vec<usize>,
    stole: bool,
}

impl Worker {
    pub fn new(
        id: usize,
        subproblem_receiver: crossbeam_channel::Receiver<SubProblem>,
        shared_ctx: Arc<RwLock<SharedContext>>,
        local_queue: crossbeam_deque::Worker<DecisionPath>,
        peer_queues: Vec<crossbeam_deque::Stealer<DecisionPath>>,
    ) -> Self {
        Worker {
            _id: id,
            subproblem_receiver,
            shared_ctx,
            local_pid: 0,
            local_problem_ctx: Arc::new(ProblemContext::default()),
            local_queue,
            peer_queues,
            offer_threshold: 0,
            level_offered_counts: Vec::new(),
            stole: false,
        }
    }

    pub fn run(&mut self) {
        while let Ok(mut subproblem) = self.subproblem_receiver.recv() {
            if subproblem.pid != self.local_pid {
                self.update_problem_ctx(&subproblem);
            }

            let ctx = Arc::clone(&self.local_problem_ctx);
            self.solve(&ctx, &mut subproblem.init_assignment);
        }
    }

    pub fn solve(&mut self, ctx: &Arc<ProblemContext>, init_assignment: &mut Vec<OptBool>) {
        let initial_assigned = init_assignment.iter().filter(|&val| val.is_some()).count();
        let mut solver =
            DPLLSolver::with_assignment(&ctx.problem, init_assignment, initial_assigned);

        let mut falsified_lit = solver.make_branching_decision();
        // info!(
        //     "[{}] Start solving PID={} @{}",
        //     self._id,
        //     self.local_pid,
        //     solver.assignment.decision_level()
        // );
        self.offer_alternative_path(&solver);
        loop {
            // Check if another worker has already found a solution
            if ctx.solution_found_flag.load(atomic::Ordering::Relaxed) {
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
                SolverAction::Decision(next_falsified_lit) => {
                    // if self.stole {
                    //     info!(
                    //         "[{}] Decision @{}",
                    //         self._id,
                    //         solver.assignment.decision_level()
                    //     );
                    // }

                    falsified_lit = next_falsified_lit;

                    // After making a decision, consider offering the decision path for the alternative branch for stealing
                    if solver.assignment.decision_level() <= self.offer_threshold {
                        self.offer_alternative_path(&solver);
                    }
                }
                SolverAction::Backtrack => {
                    // if self.stole {
                    //     info!(
                    //         "[{}] Backtrack @{}",
                    //         self._id,
                    //         solver.assignment.decision_level()
                    //     );
                    // }

                    if let Some(new_falsified_lit) = self.backtrack(&mut solver) {
                        falsified_lit = new_falsified_lit;
                        continue;
                    }

                    // Try to steal a decision path from peers
                    match self.try_steal_from_peers() {
                        Some(stolen_path) => {
                            *init_assignment = vec![OptBool::Unassigned; ctx.problem.num_vars];
                            for (var, val) in &stolen_path.decisions {
                                init_assignment[*var] = OptBool::from(*val);
                            }

                            solver = DPLLSolver::with_assignment(
                                &ctx.problem,
                                init_assignment,
                                stolen_path.decisions.len(),
                            );
                            let (last_var, last_val) = stolen_path.decisions.last().unwrap();
                            falsified_lit = Lit::new(*last_var, *last_val).negated();

                            // info!(
                            //     "[{}] Stole@{} => continue @{}",
                            //     self._id,
                            //     stolen_path.decisions.len(),
                            //     solver.assignment.decision_level()
                            // );
                        }
                        None => {
                            // Unable to backtrack & no paths to steal => We are done with this problem
                            let _ = ctx.sender.send(WorkerResult::UNSAT);
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn backtrack(&mut self, solver: &mut DPLLSolver) -> Option<Lit> {
        // Backtrack until we find a decision where the alternative path was not stolen
        loop {
            // Check if the alternative path we offered at this level was stolen.
            // We only offer paths up to offer_threshold, so only check in that range.
            if solver.assignment.decision_level() <= self.offer_threshold
                && self.local_queue.pop().is_none()
            {
                // The queue is empty => the alternative path was stolen
                return None;
            }

            // Try to backtrack one level
            match solver.assignment.backtrack_once() {
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
    }

    pub fn try_steal_from_peers(&mut self) -> Option<DecisionPath> {
        for stealer in &self.peer_queues {
            match stealer.steal() {
                crossbeam_deque::Steal::Success(path) => {
                    self.stole = true;
                    return Some(path);
                }
                _ => continue,
            }
        }
        None
    }

    pub fn update_problem_ctx(&mut self, subproblem: &SubProblem) {
        // Try to acquire read lock on shared context
        let guard = match self.shared_ctx.read() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.current_pid != subproblem.pid {
            eprintln!("Worker received subproblem with stale PID.");
            return;
        }

        self.local_problem_ctx = guard.problem_ctx.clone();
        self.local_pid = guard.current_pid;

        // Calculate offer threshold based on problem size
        let num_vars = self.local_problem_ctx.problem.num_vars;
        self.offer_threshold = (num_vars as f64).log2().ceil() as usize;
        // self.offer_threshold = 4;

        self.level_offered_counts.clear();
        self.level_offered_counts
            .resize(self.offer_threshold + 1, 0);
        self.stole = false;
    }

    pub fn offer_alternative_path(&mut self, solver: &DPLLSolver) {
        let mut decisions = solver.assignment.extract_decision();
        self.level_offered_counts[decisions.len()] += 1;

        for (level, count) in self.level_offered_counts.iter().enumerate() {
            assert!(
                *count <= 1 << (level - 1),
                "Excessive offers at level {}: {} offers",
                level,
                count
            );
        }

        // 1: T     F
        // 2: T  F  T  F
        // 3L TF TF TF TF

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
}
