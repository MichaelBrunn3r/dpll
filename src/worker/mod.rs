use crate::{clause::Lit, dpll::DPLLSolver, problem::Problem};
use std::sync::atomic::{self};

pub mod core;
pub mod stats;
pub mod stealing;

pub trait WorkerStrategy {
    fn on_new_problem(&mut self, _problem: &Problem) {}
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkerStrategyType {
    Basic,
    Stealing,
}

impl std::str::FromStr for WorkerStrategyType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "basic" => Ok(WorkerStrategyType::Basic),
            "stealing" => Ok(WorkerStrategyType::Stealing),
            _ => Err(format!(
                "Unknown strategy: {}. Use 'basic' or 'stealing'",
                s
            )),
        }
    }
}
