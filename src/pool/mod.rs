pub mod cube_and_conquer;
pub mod threaded;

use crate::{
    dpll::DPLLSolver,
    pool::{cube_and_conquer::DecisionPath, threaded::ThreadedWorkerPool},
    problem::Problem,
};
use nonzero_ext::nonzero;
use std::{num::NonZeroUsize, sync::Arc, thread::available_parallelism};

pub struct WorkerPool {
    strategy: Box<dyn WorkerPoolStrategy>,
}

impl WorkerPool {
    pub fn solve(
        problem: Arc<Problem>,
        num_workers: NonZeroUsize,
        steal: bool,
    ) -> Option<Vec<bool>> {
        match ThreadedWorkerPool::calculate_optimal_splits(&problem, num_workers) {
            Some(_) => {
                let pool = WorkerPool::new(num_workers, steal);
                pool.submit(problem)
            }
            None => {
                // Problem too small to benefit from parallelism => solve directly
                DPLLSolver::with_decisions(&problem, &DecisionPath(Vec::new())).solve()
            }
        }
    }

    pub fn new(requested_num_workers: NonZeroUsize, steal: bool) -> Self {
        // Limit number of workers to available parallelism
        let num_workers =
            requested_num_workers.min(available_parallelism().unwrap_or(nonzero!(1usize)));

        return if num_workers <= nonzero!(1usize) {
            Self {
                strategy: Box::new(SingleThreadedWorkerPool {}),
            }
        } else {
            Self {
                strategy: Box::new(ThreadedWorkerPool::new(num_workers, steal)),
            }
        };
    }

    pub fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        self.strategy.submit(problem)
    }
}

pub trait WorkerPoolStrategy {
    fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>>;
}

pub struct SingleThreadedWorkerPool {}

impl WorkerPoolStrategy for SingleThreadedWorkerPool {
    fn submit(&self, problem: Arc<Problem>) -> Option<Vec<bool>> {
        DPLLSolver::with_decisions(&problem, &DecisionPath(Vec::new())).solve()
    }
}
