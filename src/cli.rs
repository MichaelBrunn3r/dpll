// cli.rs
use dpll::constants::PROGRESS_BAR_THRESHOLD;
use dpll::utils::human_duration;
use env_logger::Env;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use log::info;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Normalizes the input path into a list of files to process.
/// Handles sorting and limit automatically.
pub fn get_problem_input_queue(path: &Path, limit: Option<usize>) -> io::Result<Vec<PathBuf>> {
    if path.is_file() {
        // Single file => Queue of size 1
        return Ok(vec![path.to_path_buf()]);
    } else if path.is_dir() {
        // Directory => Return sorted list of files
        let mut entries: Vec<PathBuf> = fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        entries.sort();

        // Limit the number of files
        if let Some(l) = limit {
            entries.truncate(l);
        }
        return Ok(entries);
    }

    // Neither file nor directory => Error
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Path is not a file or directory",
    ))
}

// ------------------
// --- CLI output ---
// ------------------

/// Initializes the logger from environment variables.
pub fn init_logging() -> MultiProgress {
    // Initialize the logger from environment variables or default to "info" level.
    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("info"));

    // Do not show timestamps/target, just the message.
    builder.format(|buf, record| writeln!(buf, "{}", record.args()));

    let logger = builder.build();
    let mp = MultiProgress::new();
    LogWrapper::new(mp.clone(), logger).try_init().unwrap();
    mp
}

/// Determines whether to use a progress bar based on estimated remaining time.
pub fn should_use_progress_bar(num_remaining: usize, sample_duration: Duration) -> bool {
    let estimated_remaining_time = sample_duration * num_remaining as u32;
    estimated_remaining_time >= PROGRESS_BAR_THRESHOLD
}

/// Creates and configures a progress bar.
/// Registers the progress bar with the provided MultiProgress.
pub fn create_progress_bar(mp: &MultiProgress, total_count: usize) -> ProgressBar {
    let pb = mp.add(ProgressBar::new(total_count as u64));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {wide_bar} [{eta_precise}]")
            .unwrap()
            .progress_chars("█▊▋▌▍▎▏░"),
    );
    pb
}

/// Formats a solution vector into a compact string representation.
pub fn format_solution_string(solution: &[bool]) -> String {
    solution
        .iter()
        .map(|&val| if val { "+" } else { "-" })
        .collect::<Vec<_>>()
        .join("")
}

// ------------------
// --- Statistics ---
// ------------------

/// Aggregated statistics.
pub struct Stats {
    pub processed: usize,
    pub errors: usize,
    pub sat_count: usize,
    pub unsat_count: usize,
    pub durations: Vec<Duration>,
    pub parse_durations: Vec<Duration>,
    pub solve_durations: Vec<Duration>,
    pub verified_count: usize,
    pub failed_verifications: usize,
}

impl Stats {
    pub fn new() -> Self {
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

    /// Prints a summary of the collected statistics.
    pub fn print_summary(&self) {
        info!("\n---\nSummary:");
        if self.verified_count > 0 || self.failed_verifications > 0 {
            info!(
                "#Files  | SAT/UNSAT/Verified | ERR\n{:<7} | {:^18} | {:^3}",
                self.processed,
                format!(
                    "{}/{}/{}",
                    self.sat_count, self.unsat_count, self.verified_count
                ),
                self.errors
            );
        } else {
            info!(
                "#Files  | SAT/UNSAT | ERR\n{:<7} | {:^9} | {:^3}",
                self.processed,
                format!("{}/{}", self.sat_count, self.unsat_count),
                self.errors
            );
        }
        self.print_duration_stats("Parsing times:", &self.parse_durations);
        self.print_duration_stats("Solving times:", &self.solve_durations);
    }

    /// Prints statistics about a list of durations.
    fn print_duration_stats(&self, title: &str, durations: &Vec<Duration>) {
        if durations.is_empty() {
            return;
        }

        let mut nanos: Vec<u128> = durations.iter().map(|d| d.as_nanos()).collect();
        nanos.sort();

        let total: u128 = nanos.iter().sum();
        let min = nanos[0];
        let max = nanos[nanos.len() - 1];
        let median = if nanos.len() % 2 == 1 {
            nanos[nanos.len() / 2]
        } else {
            let hi = nanos.len() / 2;
            (nanos[hi - 1] + nanos[hi]) / 2
        };
        let avg = total / nanos.len() as u128;

        info!(
            "\n{}\ntotal   |   min   | median  |   avg   |  max   \n{:<7} | {:^7} | {:^7} | {:^7} | {:^7}",
            title,
            human_duration(Duration::from_nanos(total as u64)),
            human_duration(Duration::from_nanos(min as u64)),
            human_duration(Duration::from_nanos(median as u64)),
            human_duration(Duration::from_nanos(avg as u64)),
            human_duration(Duration::from_nanos(max as u64)),
        );
    }
}

// ----------------------------
// --- CLI argument parsers ---
// ----------------------------

/// Parses the number of worker threads from a string.
/// Accepts either "auto" or a positive integer.
pub fn parse_num_worker_threads(s: &str) -> Result<usize, String> {
    if s == "auto" {
        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Ok(n)
    } else {
        s.parse::<usize>()
            .map_err(|_| format!("{}", s))
            .map(|n| n.max(1))
    }
}
