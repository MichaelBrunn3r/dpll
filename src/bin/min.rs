use dpll::{dpll::DPLLSolver, parser::parse_dimacs_cnf, utils::opt_bool::OptBool};
use std::io::Read;

pub fn main() -> Result<(), String> {
    let mut data = Vec::new();
    std::io::stdin()
        .read_to_end(&mut data)
        .map_err(|e| format!("Failed to read from stdin: {}", e))?;

    let problem =
        parse_dimacs_cnf(&data).map_err(|e| format!("Failed to parse DIMACS CNF input: {}", e))?;

    let assignment = vec![OptBool::Unassigned; problem.num_vars];
    let mut solver = DPLLSolver::with_assignment(&problem, assignment, 0);

    match solver.solve() {
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
