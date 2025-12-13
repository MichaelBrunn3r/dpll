use crate::{clause::Lit, dpll::DPLLSolver, problem::Problem};
use std::sync::atomic::{self};

pub mod core;
pub mod metrics;
pub mod stealing;

pub trait WorkerStrategy {
    fn on_new_problem(&mut self, _problem: &Problem) {}
    fn on_new_subproblem(&mut self, _solver: &DPLLSolver) {}
    fn after_decision(&mut self, _solver: &DPLLSolver) {}
    fn find_new_work<'p>(
        &mut self,
        _current_solver: DPLLSolver,
        _problem: &'p Problem,
        _solution_found_flag: &atomic::AtomicBool,
        _num_active_workers: &atomic::AtomicUsize,
    ) -> Option<(Lit, DPLLSolver<'p>)> {
        None
    }
    fn should_stop_backtracking_early(&mut self, _solver: &DPLLSolver) -> bool {
        false
    }
}

pub struct BasicWorker {}
impl WorkerStrategy for BasicWorker {}
