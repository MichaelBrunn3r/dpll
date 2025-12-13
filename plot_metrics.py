#! /usr/bin/env python3

import numpy as np
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import argparse
import sys
from typing import TypedDict, Optional
import numpy.typing as npt

# --- Types ---

class MetricsData(TypedDict):
    timestamps:   npt.NDArray[np.float64]
    rate_conflicts: npt.NDArray[np.int64]
    final_conflicts: int
    rate_push:    npt.NDArray[np.int64]
    rate_pop:     npt.NDArray[np.int64]
    rate_steal:   npt.NDArray[np.int64]
    rate_fail:    npt.NDArray[np.int64]
    rate_early:   npt.NDArray[np.int64]
    rate_self:    npt.NDArray[np.int64]
    avg_q_max:    npt.NDArray[np.float64]
    final_checksum: int

# Match the Rust struct layout exactly
worker_dtype = np.dtype([
    ('push', 'u8'),
    ('pop', 'u8'),
    ('steal', 'u8'),
    ('idle_micros', 'u8'),
    ('max_queue_len', 'u8'),
    ('avg_queue_len', 'f8'),
    ('early_backtracks', 'u8'),
    ('self_consumed', 'u8'),
    ('failed_steals', 'u8'),
    ('rejected_depth', 'u8'),
    ('rejected_full', 'u8'),
    ('stolen_from', 'u8'),
    ('conflicts', 'u8'),       
    ('allocated_paths', 'u8'),
])

def load_and_process_data(filename: str, max_workers: int) -> Optional[MetricsData]:
    log_row_dtype = np.dtype([
        ('timestamp_ms', 'u8'),
        ('global_allocated_paths', 'u8'),
        ('global_conflicts', 'u8'),
        ('global_path_checksum', 'u8'),
        ('workers', worker_dtype, (max_workers,))
    ])

    try:
        data = np.memmap(filename, dtype=log_row_dtype, mode='r')
    except FileNotFoundError:
        print(f"Error: File '{filename}' not found.")
        return None
    except Exception as e:
        print(f"Error reading file: {e}")
        return None

    # Filter invalid rows (timestamp 0 usually means empty/padding)
    valid_rows = data[data['timestamp_ms'] > 0]
    if len(valid_rows) == 0:
        print("No valid data found")
        return None

    timestamps = valid_rows['timestamp_ms'] / 1000.0
    workers = valid_rows['workers']

    # --- Aggregation Helper ---
    # Sums a specific field across all workers for every time step
    def sum_field(field_name):
        return np.sum(workers[field_name], axis=1)

    # --- calculate Rates (Derivatives) ---
    # We use np.diff to convert cumulative counters into "events per tick"
    
    # 1. Queue Operations
    total_push = sum_field('push')
    total_pop  = sum_field('pop')
    rate_push  = np.diff(total_push, prepend=0)
    rate_pop   = np.diff(total_pop, prepend=0)

    # 2. Stealing Operations
    total_steal = sum_field('steal')
    total_fail  = sum_field('failed_steals')
    rate_steal  = np.diff(total_steal, prepend=0)
    rate_fail   = np.diff(total_fail, prepend=0)

    # 3. Efficiency Operations
    total_early = sum_field('early_backtracks')
    total_self  = sum_field('self_consumed')
    rate_early  = np.diff(total_early, prepend=0)
    rate_self   = np.diff(total_self, prepend=0)

    # 4. Gauges (Averages, not rates)
    # Average of the "Max Queue Length" seen by workers in this tick
    avg_q_max = np.mean(workers['max_queue_len'], axis=1)

    total_conflicts_accum = valid_rows['global_conflicts']
    rate_conflicts = np.diff(total_conflicts_accum, prepend=0)
    final_checksum = valid_rows['global_path_checksum'][-1]

    return {
        "timestamps": timestamps,
        "rate_conflicts": rate_conflicts,      
        "final_conflicts": total_conflicts_accum[-1], 
        "rate_push": rate_push,
        "rate_pop": rate_pop,
        "rate_steal": rate_steal,
        "rate_fail": rate_fail,
        "rate_early": rate_early,
        "rate_self": rate_self,
        "avg_q_max": avg_q_max,
        "final_checksum": final_checksum
    }

def plot_data(metrics: MetricsData, max_workers: int) -> None:
    t = metrics["timestamps"]

    fig = make_subplots(
        rows=4, cols=1,
        shared_xaxes=True,
        vertical_spacing=0.05,
        subplot_titles=(
            "Queue Throughput (Push vs Pop)", 
            "Stealing Dynamics (Load Balancing)", 
            "Solver Efficiency (Pruning vs Brute-force)", 
            "System Load (Queue Depth)"
        )
    )

    # --- Row 1: Throughput ---
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_push"], name='Push (Production)',
        line=dict(color='#2ca02c') # Green
    ), row=1, col=1)
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_pop"], name='Pop (Consumption)',
        line=dict(color='#1f77b4') # Blue
    ), row=1, col=1)

    # --- Row 2: Stealing ---
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_steal"], name='Successful Steals',
        line=dict(color='#9467bd') # Purple
    ), row=2, col=1)
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_fail"], name='Failed Attempts',
        line=dict(color='#d62728', dash='dot') # Red dotted
    ), row=2, col=1)

    # --- Row 3: Efficiency ---
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_early"], name='Early Backtracks (Stolen)',
        line=dict(color='#e377c2') # Pink
    ), row=3, col=1)
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_self"], name='Self Consumed',
        line=dict(color='#ff7f0e') # Orange
    ), row=3, col=1)
    fig.add_trace(go.Scatter(
        x=t, y=metrics["rate_conflicts"], name='Global Conflicts/Tick',
        line=dict(color='black', width=2)
    ), row=3, col=1)

    # --- Row 4: Queue Health ---
    fig.add_trace(go.Scatter(
        x=t, y=metrics["avg_q_max"], name='Avg Max Queue Len',
        fill='tozeroy', line=dict(color='#7f7f7f') # Grey
    ), row=4, col=1)

    fig.update_layout(
        title=dict(text=f"Solver Metrics Analysis ({max_workers} Workers)"),
        hovermode="x unified",
        height=1000, # Taller for 4 rows
        legend=dict(
            orientation="h", 
            yanchor="bottom", y=1.02, 
            xanchor="right", x=1
        )
    )
    
    # Add axis labels
    fig.update_yaxes(title_text="Ops / Tick", row=1, col=1)
    fig.update_yaxes(title_text="Ops / Tick", row=2, col=1)
    fig.update_yaxes(title_text="Ops / Tick", row=3, col=1)
    fig.update_yaxes(title_text="Items", row=4, col=1)
    fig.update_xaxes(title_text="Time (s)", row=4, col=1)

    fig.show()

# --- CLI ---
parser = argparse.ArgumentParser(description="Visualize solver metrics from binary logs.")
parser.add_argument("filename", nargs="?", default="metrics.bin", help="Path to binary file")
parser.add_argument("--workers", "-w", type=int, default=16, help="Max workers (must match Rust const)")
args = parser.parse_args()

def main():
    metrics = load_and_process_data(args.filename, args.workers)
    if not metrics:
        return

    print("-" * 40)
    print(f"Total Conflicts: {metrics['final_conflicts']:,}")
    print(f"Path Checksum:   0x{metrics['final_checksum']:016X}")
    print("-" * 40)

    plot_data(metrics, args.workers)

if __name__ == "__main__":
    main()