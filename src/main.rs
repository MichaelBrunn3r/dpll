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
    /// Limit number of files to solve when PATH is a directory
    #[arg(short = 'n', long = "number", value_name = "N")]
    n: Option<usize>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let path = args.path;

    if path.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        entries.sort();

        if let Some(n) = args.n {
            entries.truncate(n);
        }

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

        stats.print_summary();
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
        problem.num_vars,
        problem.clauses.len()
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
    processed: usize,
    errors: usize,
    sat_count: usize,
    unsat_count: usize,
    durations: Vec<Duration>,
}

impl Stats {
    fn new(total_files: usize) -> Self {
        Self {
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

    fn print_summary(&self) {
        println!("\n---\nSummary:");
        println!(
            "#Files  |   SAT   |  UNSAT  |   ERR\n{:<7} | {:^7} | {:^7} | {:^7}",
            self.processed, self.sat_count, self.unsat_count, self.errors,
        );

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

            println!(
                "\ntotal   |   min   | median  |   avg   |  max   \n{:<7} | {:^7} | {:^7} | {:^7} | {:^7}",
                human_duration(Duration::from_secs_f64(total_secs)),
                human_duration(Duration::from_secs_f64(min)),
                human_duration(Duration::from_secs_f64(median)),
                human_duration(Duration::from_secs_f64(avg_secs)),
                human_duration(Duration::from_secs_f64(max)),
            );
        }
    }
}
