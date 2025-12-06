use crate::cli::{self, Stats};
use dpll::{
    measure_time, parser::parse_dimacs_cnf, pool::WorkerPool, record_time, utils::human_duration,
    worker::WorkerStrategyType,
};
use log::{error, info, warn};
use memmap2::Mmap;
use std::{
    error::Error,
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

pub fn solve(
    path: PathBuf,
    limit: Option<usize>,
    validate: bool,
    num_worker_threads: usize,
    no_progress_bar: bool,
    strategy: WorkerStrategyType,
) -> Result<(), Box<dyn Error>> {
    let progress = cli::init_logging();

    let start = Instant::now();
    let pool = WorkerPool::new(num_worker_threads, strategy);
    info!(
        "Initialized pool with {} worker thread(s).",
        pool.num_workers
    );

    let mut stats = Stats::new();
    let mut queue = cli::get_problem_input_queue(&path, limit)?;

    // Process the first file to estimate the remaining runtime
    let first_file = if let Some(f) = queue.pop() {
        f
    } else {
        return Ok(());
    };
    let first_duration = measure_time!({
        solve_file(&first_file, &pool, &mut stats, validate).map_err(|e| {
            error!("Error while solving {:?}: {}", first_file, e);
            e
        })?
    });

    if !queue.is_empty() {
        // Create a progress bar if the remaining time is significant enough
        let pb = if !no_progress_bar && cli::should_use_progress_bar(queue.len(), first_duration) {
            let pb = cli::create_progress_bar(&progress, queue.len());
            pb.set_position(1); // Account for the first file we just solved
            Some(pb)
        } else {
            None
        };

        // Process the remaining files
        for path in queue {
            if let Err(e) = solve_file(&path, &pool, &mut stats, validate) {
                error!("Error while solving {:?}: {}", path, e);
            }
            pb.as_ref().map(|p| p.inc(1));
        }
        pb.as_ref().map(|p| p.finish_with_message("done"));
    }

    stats.print_summary();
    info!("Total runtime: {}", human_duration(start.elapsed()));

    Ok(())
}

/// Solves a single DIMACS CNF file, updating stats and optionally verifying the solution.
fn solve_file(
    path: &Path,
    pool: &WorkerPool,
    stats: &mut Stats,
    validate_solution: bool,
) -> Result<Option<Vec<bool>>, Box<dyn Error>> {
    info!("Solving {:?}", path);
    stats.processed += 1;

    // Parse the problem
    let problem = {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        record_time!(stats.parse_durations, {
            Arc::new(parse_dimacs_cnf(&mmap)?)
        })
    };

    // Solve the problem
    if let Some(solution) = record_time!(stats.solve_durations, { pool.submit(problem.clone()) }) {
        stats.sat_count += 1;

        // Validate the solution
        if validate_solution {
            if let Err(msg) = problem.verify_solution(&solution) {
                warn!("Solution verification failed: {}", msg);
                stats.failed_verifications += 1;
            } else {
                stats.verified_count += 1;
            }
        }

        info!("SAT {}", cli::format_solution_string(&solution));
        return Ok(Some(solution));
    } else {
        stats.unsat_count += 1;
        info!("UNSAT");
        return Ok(None);
    }
}
