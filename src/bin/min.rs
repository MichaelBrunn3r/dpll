use dpll::{parser::parse_dimacs_cnf, pool::WorkerPool};
use nonzero_ext::nonzero;
use std::{io::Read, sync::Arc, thread::available_parallelism};

pub fn main() -> Result<(), String> {
    let mut data = Vec::new();
    std::io::stdin()
        .read_to_end(&mut data)
        .map_err(|e| format!("Failed to read from stdin: {}", e))?;

    let problem =
        parse_dimacs_cnf(&data).map_err(|e| format!("Failed to parse DIMACS CNF input: {}", e))?;

    match WorkerPool::solve(
        Arc::new(problem),
        available_parallelism().unwrap_or(nonzero!(1usize)),
        false,
    ) {
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
