use dpll::{
    dpll::DPLLSolver,
    parser::parse_dimacs_cnf,
    pool::{DecisionPath, WorkerPool},
    problem::Problem,
};
use std::{io::Read, num::NonZero, sync::Arc, thread::available_parallelism};

pub fn main() -> Result<(), String> {
    let mut data = Vec::new();
    std::io::stdin()
        .read_to_end(&mut data)
        .map_err(|e| format!("Failed to read from stdin: {}", e))?;

    let problem =
        parse_dimacs_cnf(&data).map_err(|e| format!("Failed to parse DIMACS CNF input: {}", e))?;

    // let solution = solve_single_threaded(problem);
    let solution = solve_multi_threaded(problem);

    match solution {
        None => {
            println!("s UNSATISFIABLE");
        }
        Some(result) => {
            println!("s SATISFIABLE");
            print!("v");
            for (var_index, value) in result.iter().enumerate() {
                match value {
                    true => print!(" {}", var_index + 1),
                    false => print!(" -{}", var_index + 1),
                }
            }
            println!();
        }
    }

    Ok(())
}

pub fn solve_single_threaded(problem: Problem) -> Option<Vec<bool>> {
    DPLLSolver::with_decisions(&problem, &DecisionPath(Vec::new())).solve()
}

pub fn solve_multi_threaded(problem: Problem) -> Option<Vec<bool>> {
    let num_workers = available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();
    let pool = WorkerPool::new(num_workers, false);
    pool.submit(Arc::new(problem))
}
