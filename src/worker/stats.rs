use comfy_table::{Cell, Color, ContentArrangement, Table};
use crossbeam_deque::Worker;
use crossbeam_utils::CachePadded;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

pub const MAX_WORKERS: usize = 16;
#[cfg(feature = "stats")]
pub static WORKER_STATS: [CachePadded<WorkerStats>; MAX_WORKERS] =
    [const { CachePadded::new(WorkerStats::new()) }; MAX_WORKERS];
#[cfg(feature = "stats")]
pub static PEER_STATS: [CachePadded<WorkerPeerStats>; MAX_WORKERS] =
    [const { CachePadded::new(WorkerPeerStats::new()) }; MAX_WORKERS];

/// Per-worker statistics.
#[derive(Default)]
pub struct WorkerStats {
    pub push: AtomicU64,
    pub pop: AtomicU64,
    pub steal: AtomicU64,
    pub idle_micros: AtomicU64,
}

impl WorkerStats {
    pub const fn new() -> Self {
        WorkerStats {
            push: AtomicU64::new(0),
            pop: AtomicU64::new(0),
            steal: AtomicU64::new(0),
            idle_micros: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct WorkerStatsSnapshot {
    pub push: u64,
    pub pop: u64,
    pub steal: u64,
    pub idle_micros: u64,
}

impl WorkerStatsSnapshot {
    pub const fn new() -> Self {
        WorkerStatsSnapshot {
            push: 0,
            pop: 0,
            steal: 0,
            idle_micros: 0,
        }
    }
}

/// Per-worker statistics interacting with peers.
#[derive(Default)]
pub struct WorkerPeerStats {
    pub stolen_from: AtomicU64,
}

impl WorkerPeerStats {
    pub const fn new() -> Self {
        WorkerPeerStats {
            stolen_from: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct WorkerPeerStatsSnapshot {
    pub stolen_from: u64,
}

impl WorkerPeerStatsSnapshot {
    pub const fn new() -> Self {
        WorkerPeerStatsSnapshot { stolen_from: 0 }
    }
}

#[cfg(feature = "stats")]
pub fn print_worker_stats_summary(
    num_workers: usize,
    stats_snapshots: &mut Vec<WorkerStatsSnapshot>,
    peer_snapshots: &mut Vec<WorkerPeerStatsSnapshot>,
) {
    if stats_snapshots.len() < num_workers {
        stats_snapshots.resize(num_workers, WorkerStatsSnapshot::new());
    }
    if peer_snapshots.len() < num_workers {
        peer_snapshots.resize(num_workers, WorkerPeerStatsSnapshot::new());
    }

    // --- ANSI Colors ---
    const GREEN: &str = "\x1b[32m";
    const RESET: &str = "\x1b[0m";

    // --- Formatters ---

    let fmt_combined =
        |push: u64, d_push: u64, pop: u64, d_pop: u64, stolen: u64, d_stolen: u64| -> String {
            let base = format!("{}/{}/{}", push, pop, stolen);
            if d_push > 0 || d_pop > 0 || d_stolen > 0 {
                format!(
                    "{} {}+{}/{}/{}{}",
                    base, GREEN, d_push, d_pop, d_stolen, RESET
                )
            } else {
                base
            }
        };

    let fmt_count = |val: u64, delta: u64| -> String {
        if delta > 0 {
            format!("{} {}+{}{}", val, GREEN, delta, RESET)
        } else {
            val.to_string()
        }
    };

    let fmt_time = |micros: u64, delta_micros: u64| -> String {
        let val = format_duration(micros);
        if delta_micros > 0 {
            let delta = format_duration(delta_micros);
            format!("{} {}+{}{}", val, GREEN, delta, RESET)
        } else {
            val
        }
    };

    // --- Table Configuration ---
    let mut table = Table::new();

    table
        .load_preset(comfy_table::presets::UTF8_HORIZONTAL_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["ID", "Push/Pop/Stolen", "Steal", "Idle"]);

    for column in table.column_iter_mut() {
        column.set_padding((0, 0));
    }

    // Totals for local stats
    let mut t_push = 0;
    let mut t_p_delta = 0;
    let mut t_pop = 0;
    let mut t_o_delta = 0;
    let mut t_steal = 0;
    let mut t_s_delta = 0;
    let mut t_idle = 0;
    let mut t_i_delta = 0;

    // Totals for peer stats
    let mut t_stolen_from = 0;
    let mut t_sf_delta = 0;

    let limit = num_workers.min(crate::worker::stats::MAX_WORKERS);

    for i in 0..limit {
        let slot = &crate::worker::stats::WORKER_STATS[i];
        let peer_slot = &crate::worker::stats::PEER_STATS[i];

        // --- Local Stats ---
        let cur_push = slot.push.load(std::sync::atomic::Ordering::Relaxed);
        let cur_pop = slot.pop.load(std::sync::atomic::Ordering::Relaxed);
        let cur_steal = slot.steal.load(std::sync::atomic::Ordering::Relaxed);
        let cur_idle = slot.idle_micros.load(std::sync::atomic::Ordering::Relaxed);

        let d_push = cur_push.saturating_sub(stats_snapshots[i].push);
        let d_pop = cur_pop.saturating_sub(stats_snapshots[i].pop);
        let d_steal = cur_steal.saturating_sub(stats_snapshots[i].steal);
        let d_idle = cur_idle.saturating_sub(stats_snapshots[i].idle_micros);

        stats_snapshots[i].push = cur_push;
        stats_snapshots[i].pop = cur_pop;
        stats_snapshots[i].steal = cur_steal;
        stats_snapshots[i].idle_micros = cur_idle;

        // --- Peer Stats (Stolen From) ---
        let cur_sf = peer_slot
            .stolen_from
            .load(std::sync::atomic::Ordering::Relaxed);
        let d_sf = cur_sf.saturating_sub(peer_snapshots[i].stolen_from);

        peer_snapshots[i].stolen_from = cur_sf;

        // --- Accumulate Totals ---
        t_push += cur_push;
        t_p_delta += d_push;
        t_pop += cur_pop;
        t_o_delta += d_pop;
        t_steal += cur_steal;
        t_s_delta += d_steal;
        t_idle += cur_idle;
        t_i_delta += d_idle;

        t_stolen_from += cur_sf;
        t_sf_delta += d_sf;

        // --- Format & Add Row ---
        let s_combined = fmt_combined(cur_push, d_push, cur_pop, d_pop, cur_sf, d_sf);
        let s_steal = fmt_count(cur_steal, d_steal);
        let s_idle = fmt_time(cur_idle, d_idle);

        table.add_row(vec![
            Cell::new(i),
            Cell::new(s_combined),
            Cell::new(s_steal),
            Cell::new(s_idle),
        ]);
    }

    // --- Totals Row ---
    let tot_combined = fmt_combined(
        t_push,
        t_p_delta,
        t_pop,
        t_o_delta,
        t_stolen_from,
        t_sf_delta,
    );
    let tot_steal = fmt_count(t_steal, t_s_delta);
    let tot_idle = fmt_time(t_idle, t_i_delta);

    table.add_row(vec![
        Cell::new("Σ"),
        Cell::new(tot_combined),
        Cell::new(tot_steal),
        Cell::new(tot_idle),
    ]);

    println!("\n{table}");
}

fn format_duration(micros: u64) -> String {
    if micros >= 60_000_000 {
        // Minutes
        format!("{:.0}m", micros as f64 / 60_000_000.0)
    } else if micros >= 1_000_000 {
        // Seconds
        format!("{:.0}s", micros as f64 / 1_000_000.0)
    } else if micros >= 1_000 {
        // Milliseconds
        format!("{:.0}ms", micros as f64 / 1_000.0)
    } else {
        // Microseconds (already whole)
        format!("{}µs", micros)
    }
}

#[cfg(feature = "stats")]
#[macro_export]
macro_rules! stats {
    // Matches: stats!(id, |local, peer| { ... })
    ($id:expr, |$local:ident, $peers:ident| $code:block) => {{
        let $local = unsafe { crate::worker::stats::WORKER_STATS.get_unchecked($id) };
        let $peers = &crate::worker::stats::PEER_STATS;
        $code
    }};
}

#[cfg(not(feature = "stats"))]
#[macro_export]
macro_rules! stats {
    ($id:expr, |$local:ident, $peers:ident| $code:block) => {};
}
