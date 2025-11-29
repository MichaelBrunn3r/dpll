use clap::Parser;
use dpll::parser::parse_dimacs_cnf;
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
    /// Limit to N files when processing a directory
    #[arg(short = 'l', long = "limit", value_name = "LIMIT")]
    limit: Option<usize>,
    /// Verify the solution
    #[arg(long)]
    verify: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let path = args.path;
    let mut stats = &mut Stats::new();

    if path.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        entries.sort();

        if let Some(limit) = args.limit {
            entries.truncate(limit);
        }

        for entry in entries {
            println!();
            match solve_file(&entry, args.verify, &mut stats) {
                Err(e) => {
                    stats.errors += 1;
                    eprintln!("Error solving {:?}: {}", entry, e);
                }
                _ => {}
            }
        }
    } else if path.is_file() {
        solve_file(&path, args.verify, stats)?;
    } else {
        return Err(format!("Path {:?} is not a file or directory", path).into());
    }

    stats.print_summary();
    Ok(())
}

fn solve_file(path: &Path, verify: bool, stats: &mut Stats) -> Result<(), Box<dyn Error>> {
    println!("---\nProcessing file: {:?}\n---", &path);
    stats.processed += 1;

    let start = Instant::now();
    let file = File::open(path)?;
    // SAFETY: mapping a file is safe as long as the file isn't modified concurrently.
    let mmap = unsafe { Mmap::map(&file)? };

    let parse_start = Instant::now();
    let problem = parse_dimacs_cnf(&mmap)?;
    let parse_elapsed = parse_start.elapsed();
    stats.parse_durations.push(parse_elapsed);
    println!(
        "Problem: {} variables, {} clauses",
        problem.num_vars,
        problem.num_clauses()
    );

    let solve_start = Instant::now();
    match &problem.solve() {
        Some(solution) => {
            stats.sat_count += 1;
            let solve_elapsed = solve_start.elapsed();
            let total_elapsed = start.elapsed();
            stats.solve_durations.push(solve_elapsed);
            stats.durations.push(total_elapsed);

            print!("SAT! ");
            for (i, val) in solution.iter().enumerate() {
                if *val {
                    print!("{} ", i + 1);
                } else {
                    print!("¬{} ", i + 1);
                }
            }

            if verify {
                match problem.verify(solution) {
                    Ok(()) => {
                        print!("OK");
                        stats.verified_count += 1;
                    }
                    Err(msg) => {
                        print!("FAIL {} ", msg);
                        stats.failed_verifications += 1;
                    }
                };
            }

            println!();
        }
        None => {
            stats.unsat_count += 1;
            let solve_elapsed = solve_start.elapsed();
            let total_elapsed = start.elapsed();
            stats.solve_durations.push(solve_elapsed);
            stats.durations.push(total_elapsed);
            println!("UNSAT");
        }
    }

    Ok(())
}

/// Aggregated statistics for a directory run.
struct Stats {
    processed: usize,
    errors: usize,
    sat_count: usize,
    unsat_count: usize,
    durations: Vec<Duration>,
    parse_durations: Vec<Duration>,
    solve_durations: Vec<Duration>,
    verified_count: usize,
    failed_verifications: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            processed: 0,
            errors: 0,
            sat_count: 0,
            unsat_count: 0,
            durations: Vec::new(),
            parse_durations: Vec::new(),
            solve_durations: Vec::new(),
            verified_count: 0,
            failed_verifications: 0,
        }
    }

    fn print_summary(&self) {
        println!("\n---\nSummary:");

        if self.verified_count > 0 || self.failed_verifications > 0 {
            println!(
                "#Files  | SAT/UNSAT/Verified | ERR\n{:<7} | {:^18} | {:^3}",
                self.processed,
                format!(
                    "{}/{}/{}",
                    self.sat_count, self.unsat_count, self.verified_count
                ),
                self.errors
            );
        } else {
            println!(
                "#Files  | SAT/UNSAT | ERR\n{:<7} | {:^9} | {:^3}",
                self.processed,
                format!("{}/{}", self.sat_count, self.unsat_count),
                self.errors
            );
        }

        self.print_times_table("Parsing times:", &self.parse_durations);
        self.print_times_table("Solving times:", &self.solve_durations);
        self.print_times_table("Total times:", &self.durations);
    }

    fn print_times_table(&self, title: &str, durations: &Vec<Duration>) {
        if durations.is_empty() {
            return;
        }
        let total_secs: f64 = durations.iter().map(|d| d.as_secs_f64()).sum();
        let avg_secs = total_secs / (durations.len() as f64);
        let mut secs_vec: Vec<f64> = durations.iter().map(|d| d.as_secs_f64()).collect();
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
            "\n{}\ntotal   |   min   | median  |   avg   |  max   \n{:<7} | {:^7} | {:^7} | {:^7} | {:^7}",
            title,
            human_duration(Duration::from_secs_f64(total_secs)),
            human_duration(Duration::from_secs_f64(min)),
            human_duration(Duration::from_secs_f64(median)),
            human_duration(Duration::from_secs_f64(avg_secs)),
            human_duration(Duration::from_secs_f64(max)),
        );
    }
}
