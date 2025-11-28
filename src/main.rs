use clap::Parser;
use dpll::parser::parse_dimacs;
use dpll::utils::human_duration;
use memmap2::Mmap;
use std::time::{Duration, Instant};

use std::{
    error::Error,
    fs,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
#[command(author, version, long_about = None)]
struct Args {
    #[arg(value_name = "PATH")]
    path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let path = args.path;

    if path.is_dir() {
        println!("Solving directory {:?}", &path);
        let mut entries: Vec<PathBuf> = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        entries.sort();

        let mut stats = Stats::new(entries.len());

        for entry in entries {
            let start = Instant::now();
            println!("\n---\nProcessing file: {:?}\n---", &entry);
            match solve_file(&entry) {
                Ok(opt_solution) => {
                    let elapsed = start.elapsed();
                    stats.record_success(opt_solution, elapsed);
                    println!("Elapsed: {}", human_duration(elapsed));
                }
                Err(e) => {
                    stats.errors += 1;
                    eprintln!("Error solving {:?}: {}", entry, e);
                }
            }
        }

        stats.print_summary(&path);
    } else if path.is_file() {
        let start = Instant::now();
        println!("---\nProcessing file: {:?}\n---", &path);
        solve_file(&path)?;
        let elapsed = start.elapsed();
        println!("Elapsed: {}", human_duration(elapsed));
    } else {
        return Err(format!("Path {:?} is not a file or directory", path).into());
    }

    Ok(())
}

fn solve_file(path: &Path) -> Result<Option<Vec<bool>>, Box<dyn Error>> {
    let file = File::open(path)?;
    // SAFETY: mapping a file is safe as long as the file isn't modified concurrently.
    let mmap = unsafe { Mmap::map(&file)? };

    let problem = parse_dimacs(&mmap)?;
    println!(
        "Problem: {} variables, {} clauses",
        problem.num_vars, problem.num_clauses
    );
    let res = problem.solve();
    match &res {
        Some(solution) => {
            print!("SAT! ");
            for (i, val) in solution.iter().enumerate() {
                if *val {
                    print!("{} ", i + 1);
                } else {
                    print!("¬{} ", i + 1);
                }
            }
            println!();
        }
        None => println!("UNSAT"),
    }

    Ok(res)
}

/// Aggregated statistics for a directory run.
struct Stats {
    total_files: usize,
    processed: usize,
    errors: usize,
    sat_count: usize,
    unsat_count: usize,
    durations: Vec<Duration>,
}

impl Stats {
    fn new(total_files: usize) -> Self {
        Self {
            total_files,
            processed: 0,
            errors: 0,
            sat_count: 0,
            unsat_count: 0,
            durations: Vec::with_capacity(total_files),
        }
    }

    fn record_success(&mut self, solution: Option<Vec<bool>>, dur: Duration) {
        self.processed += 1;
        self.durations.push(dur);
        if solution.is_some() {
            self.sat_count += 1;
        } else {
            self.unsat_count += 1;
        }
    }

    fn print_summary(&self, path: &std::path::Path) {
        println!("\nSummary for directory {:?}", path);
        println!("  files found: {}", self.total_files);
        println!("  processed: {}", self.processed);
        println!("  errors: {}", self.errors);
        println!("  SAT: {}, UNSAT: {}", self.sat_count, self.unsat_count);

        if !self.durations.is_empty() {
            let total_secs: f64 = self.durations.iter().map(|d| d.as_secs_f64()).sum();
            let avg_secs = total_secs / (self.durations.len() as f64);
            let mut secs_vec: Vec<f64> = self.durations.iter().map(|d| d.as_secs_f64()).collect();
            secs_vec.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let min = secs_vec.first().cloned().unwrap_or(0.0);
            let max = secs_vec.last().cloned().unwrap_or(0.0);
            let median = if secs_vec.len() % 2 == 1 {
                secs_vec[secs_vec.len() / 2]
            } else {
                let hi = secs_vec.len() / 2;
                (secs_vec[hi - 1] + secs_vec[hi]) / 2.0
            };

            let total_dur = Duration::from_secs_f64(total_secs);
            let avg_dur = Duration::from_secs_f64(avg_secs);
            let min_dur = Duration::from_secs_f64(min);
            let max_dur = Duration::from_secs_f64(max);
            let median_dur = Duration::from_secs_f64(median);

            println!("  total time: {}", human_duration(total_dur));
            println!("  avg time:   {}", human_duration(avg_dur));
            println!("  min time:   {}", human_duration(min_dur));
            println!("  median:     {}", human_duration(median_dur));
            println!("  max time:   {}", human_duration(max_dur));
        }
    }
}
