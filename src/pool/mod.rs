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
        let optimal_num_workers = decide_num_workers(&problem, num_workers);
        if optimal_num_workers <= nonzero!(1usize) {
            return DPLLSolver::with_decisions(&problem, &DecisionPath(Vec::new())).solve();
        }

        let pool = WorkerPool::new(num_workers, steal);
        pool.submit(problem)
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

pub fn decide_num_workers(problem: &Problem, max_workers: NonZeroUsize) -> NonZeroUsize {
    let mut num_workers = nonzero!(1usize);

    if problem.num_vars >= 250 {
        num_workers = nonzero!(16usize);
    } else if problem.num_vars >= 200 {
        num_workers = nonzero!(12usize);
    } else if problem.num_vars >= 100 {
        num_workers = nonzero!(8usize);
    }

    return num_workers.min(max_workers);
}
