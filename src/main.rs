use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, long_about = None)]
struct Args {
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

fn main() {
    let args = Args::parse();
    let buf = fs::read(&args.file).expect("Failed to read file");
    println!("Solving {:?}", args.file);

    match dpll::parser::parse_dimacs(&buf) {
        Ok((problem, clauses)) => {
            println!(
                "Problem: {} variables, {} clauses",
                problem.num_vars, problem.num_clauses
            );
            println!("Parsed {} clauses.", clauses.len());
            println!(
                "{}",
                clauses
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        Err(e) => panic!("Parse error: {}", e),
    }
}
