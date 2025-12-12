#[cfg(feature = "metrics")]
use crossbeam_utils::CachePadded;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
#[cfg(feature = "metrics")]
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    sync::atomic::Ordering,
    time::Instant,
};

// -------------
// --- State ---
// -------------

pub const MAX_WORKERS: usize = 16;

#[cfg(feature = "metrics")]
static SHARED_METRICS: CachePadded<SharedMetrics> = CachePadded::new(SharedMetrics::new());

#[cfg(feature = "metrics")]
static PER_WORKER_METRICS: [CachePadded<WorkerMetrics>; MAX_WORKERS] =
    [const { CachePadded::new(WorkerMetrics::new()) }; MAX_WORKERS];

#[cfg(feature = "metrics")]
static INTERACTION_STATS: [CachePadded<InteractionMetrics>; MAX_WORKERS] =
    [const { CachePadded::new(InteractionMetrics::new()) }; MAX_WORKERS];

// ----------------------
// --- Record metrics ---
// ----------------------

/// Record that a worker successfully stole a path from another worker
#[inline(always)]
#[allow(unused)]
pub fn record_stole_from(thief: usize, victim: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[thief]
            .steal
            .fetch_add(1, Ordering::Relaxed);
        INTERACTION_STATS[victim]
            .stolen_from
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record that a worker attempted to steal but found no work
#[inline(always)]
#[allow(unused)]
pub fn record_failed_to_steal(worker_id: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .failed_steals
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record that a worker had its work stolen by another worker and thus stopped backtracking early
#[inline(always)]
#[allow(unused)]
pub fn record_work_was_stolen(worker_id: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .early_backtracks
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record that a path was allocated because the local pool was empty
#[inline(always)]
pub fn record_allocated_path() {
    #[cfg(feature = "metrics")]
    {
        SHARED_METRICS
            .allocated_paths
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record the current length of a worker's local queue
#[inline(always)]
#[allow(unused)]
pub fn record_queue_length(worker_id: usize, len: u64) {
    #[cfg(feature = "metrics")]
    {
        let metrics = &PER_WORKER_METRICS[worker_id];

        // Update Max
        let current_max = metrics.max_queue_len.load(Ordering::Relaxed);
        if len > current_max {
            metrics.max_queue_len.store(len, Ordering::Relaxed);
        }

        // Update Running Average
        if len > 0 {
            let len_f = len as f64;
            const ALPHA: f64 = 0.01;
            let old_bits = metrics.avg_non_empty_queue_len.load(Ordering::Relaxed);
            let new_avg = if old_bits == 0 {
                len_f
            } else {
                let old_val = f64::from_bits(old_bits);
                old_val + ALPHA * (len_f - old_val)
            };
            metrics
                .avg_non_empty_queue_len
                .store(new_avg.to_bits(), Ordering::Relaxed);
        }
    }
}

/// Record that a path was not put into the queue due to exceeding the offer threshold
#[inline(always)]
#[allow(unused)]
pub fn record_path_exceeds_offer_threshold(worker_id: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .rejected_depth
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record that a worker pushed a path into its local queue
#[inline(always)]
#[allow(unused)]
pub fn record_push_into_queue(worker_id: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .push
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record that a worker popped a path from its local queue
#[inline(always)]
#[allow(unused)]
pub fn record_pop_from_queue(worker_id: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .pop
            .fetch_add(1, Ordering::Relaxed);
        PER_WORKER_METRICS[worker_id]
            .self_consumed
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Record that no path was pushed into the queue because it was full
#[inline(always)]
#[allow(unused)]
pub fn record_queue_full(worker_id: usize) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .rejected_full
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
#[allow(unused)]
pub fn record_idle_for(worker_id: usize, micros: u64) {
    #[cfg(feature = "metrics")]
    {
        PER_WORKER_METRICS[worker_id]
            .idle_micros
            .fetch_add(micros, Ordering::Relaxed);
    }
}

// --------------
// --- Macros ---
// --------------

/// Macro to execute a block of code only if metrics are enabled
#[cfg(feature = "metrics")]
#[macro_export]
macro_rules! if_metrics {
    ($($body:tt)*) => {
        $($body)*
    };
}

/// Macro to execute a block of code only if metrics are enabled
#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! if_metrics {
    ($($body:tt)*) => {};
}

// --------------
// --- Logger ---
// --------------

/// Logger that periodically writes metrics to a binary file
#[cfg(feature = "metrics")]
pub struct MetricsLogger {
    start_time: Instant,
    prev_tick: Instant,
    tick_rate: Duration,
    pub filename: String,
    writer: BufWriter<File>,
}

/// Logger that periodically writes metrics to a binary file
#[cfg(not(feature = "metrics"))]
pub struct MetricsLogger {}

/// Single row containing all metrics during a given tick
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LogRow {
    pub timestamp_ms: u64,
    pub global_allocated_paths: u64,
    pub workers: [WorkerLogData; MAX_WORKERS],
}

/// Per-worker metrics data in a log row
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WorkerLogData {
    pub push: u64,
    pub pop: u64,
    pub steal: u64,
    pub idle_micros: u64,
    pub max_queue_len: u64,
    pub avg_queue_len: f64, // Decoded from bits
    pub early_backtracks: u64,
    pub self_consumed: u64,
    pub failed_steals: u64,
    pub rejected_depth: u64,
    pub rejected_full: u64,
    pub stolen_from: u64, // From PEER_STATS
}

#[cfg(feature = "metrics")]
impl MetricsLogger {
    pub fn new(filename: &str, tick_rate: Duration) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(filename)?;

        Ok(Self {
            start_time: Instant::now(),
            prev_tick: Instant::now(),
            tick_rate,
            filename: filename.to_string(),
            writer: BufWriter::new(file),
        })
    }

    /// Captures metrics if a tick is due
    pub fn tick(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.prev_tick) <= self.tick_rate {
            return;
        }

        if let Err(e) = self.capture() {
            eprintln!("Failed to capture metrics: {}", e);
        }

        self.prev_tick = now;
    }

    /// Captures current metrics and write to file
    fn capture(&mut self) -> std::io::Result<()> {
        let elapsed = self.start_time.elapsed().as_millis() as u64;

        let mut row = LogRow {
            timestamp_ms: elapsed,
            global_allocated_paths: SHARED_METRICS.allocated_paths.load(Ordering::Relaxed),
            workers: [WorkerLogData::default(); MAX_WORKERS],
        };

        // Gather per-worker metrics
        for i in 0..MAX_WORKERS {
            let w_stats = &PER_WORKER_METRICS[i];
            let p_stats = &INTERACTION_STATS[i];

            let max_queue_len = w_stats.max_queue_len.swap(0, Ordering::Relaxed);
            let avg_queue_len = if max_queue_len == 0 {
                0.0
            } else {
                f64::from_bits(w_stats.avg_non_empty_queue_len.load(Ordering::Relaxed))
            };

            row.workers[i] = WorkerLogData {
                push: w_stats.push.load(Ordering::Relaxed),
                pop: w_stats.pop.load(Ordering::Relaxed),
                steal: w_stats.steal.load(Ordering::Relaxed),
                idle_micros: w_stats.idle_micros.load(Ordering::Relaxed),
                max_queue_len,
                avg_queue_len,
                early_backtracks: w_stats.early_backtracks.load(Ordering::Relaxed),
                self_consumed: w_stats.self_consumed.load(Ordering::Relaxed),
                failed_steals: w_stats.failed_steals.load(Ordering::Relaxed),
                rejected_depth: w_stats.rejected_depth.load(Ordering::Relaxed),
                rejected_full: w_stats.rejected_full.load(Ordering::Relaxed),
                stolen_from: p_stats.stolen_from.load(Ordering::Relaxed),
            };
        }

        // Write the row as raw bytes
        let ptr = &row as *const LogRow as *const u8;
        let bytes = unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of::<LogRow>()) };
        self.writer.write_all(bytes)?;

        Ok(())
    }

    /// Finalizes the logger and returns the filename
    pub fn close(mut self) -> std::io::Result<String> {
        // One final capture before closing
        let _ = self.capture();
        self.writer.flush().map(|_| self.filename)
    }
}

#[cfg(not(feature = "metrics"))]
impl MetricsLogger {
    // Adjusted signature to match stats version to prevent conditional compilation errors
    pub fn new(_filename: impl AsRef<Path>, _tick_rate: Duration) -> std::io::Result<Self> {
        Ok(Self {})
    }

    pub fn tick(&mut self) {
        // No-op
    }

    pub fn close(self) -> std::io::Result<()> {
        Ok(())
    }
}

// -----------------------
// --- Metrics structs ---
// -----------------------

/// Per-worker metrics
#[derive(Default)]
pub struct WorkerMetrics {
    pub push: AtomicU64,
    pub pop: AtomicU64,
    pub steal: AtomicU64,
    pub idle_micros: AtomicU64,
    pub max_queue_len: AtomicU64,
    pub avg_non_empty_queue_len: AtomicU64, // Stored as f64 bits
    pub early_backtracks: AtomicU64,
    pub self_consumed: AtomicU64,
    pub failed_steals: AtomicU64,
    pub rejected_depth: AtomicU64,
    pub rejected_full: AtomicU64,
}

impl WorkerMetrics {
    pub const fn new() -> Self {
        WorkerMetrics {
            push: AtomicU64::new(0),
            pop: AtomicU64::new(0),
            steal: AtomicU64::new(0),
            idle_micros: AtomicU64::new(0),
            max_queue_len: AtomicU64::new(0),
            avg_non_empty_queue_len: AtomicU64::new(0),
            early_backtracks: AtomicU64::new(0),
            self_consumed: AtomicU64::new(0),
            failed_steals: AtomicU64::new(0),
            rejected_depth: AtomicU64::new(0),
            rejected_full: AtomicU64::new(0),
        }
    }
}

/// Metrics shared across all workers
pub struct SharedMetrics {
    pub allocated_paths: AtomicU64,
}

impl SharedMetrics {
    pub const fn new() -> Self {
        SharedMetrics {
            allocated_paths: AtomicU64::new(0),
        }
    }
}

/// Metrics of interactions between workers
#[derive(Default)]
pub struct InteractionMetrics {
    pub stolen_from: AtomicU64,
}

impl InteractionMetrics {
    pub const fn new() -> Self {
        InteractionMetrics {
            stolen_from: AtomicU64::new(0),
        }
    }
}
