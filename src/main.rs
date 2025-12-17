pub mod cli;

use crate::cli::{generate::generate, solve::solve};
use clap::{Parser, Subcommand};
use std::{error::Error, num::NonZeroUsize, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Solve {
        /// Path to a file or directory of DIMACS CNF problem instances
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Limit the number of problems to solve
        #[arg(short = 'l', long = "limit", value_name = "LIMIT")]
        limit: Option<usize>,
        /// Validate solutions after solving
        #[arg(long)]
        validate: bool,
        /// Number of worker threads to use (number or 'auto')
        #[arg(short = 'w', long = "worker-threads", value_name = "N", default_value = "1", value_parser = cli::parse_num_worker_threads)]
        num_worker_threads: NonZeroUsize,
        /// Disable the progress bar
        #[arg(long = "no-bar")]
        no_progress_bar: bool,
        #[arg(short = 's', long = "steal", default_value_t = false)]
        steal: bool,
    },
    #[command(name = "generate")]
    Generate { num_pigeons: usize },
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    match args.command {
        Command::Solve {
            path,
            limit,
            validate,
            num_worker_threads,
            no_progress_bar,
            steal,
        } => {
            solve(
                path,
                limit,
                validate,
                num_worker_threads,
                no_progress_bar,
                steal,
            )?;
        }
        Command::Generate { num_pigeons: size } => {
            generate(size)?;
        }
    }

    Ok(())
}
