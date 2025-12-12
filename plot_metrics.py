#! /usr/bin/env python3

import numpy as np
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import argparse
import sys
from typing import TypedDict, Optional
import numpy.typing as npt

class MetricsData(TypedDict):
    timestamps: npt.NDArray[np.float64]
    rate_early: npt.NDArray[np.int64]
    rate_self:  npt.NDArray[np.int64]
    avg_q_max:  npt.NDArray[np.float64]

# Data structure for per-worker metrics in a log row
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
])

def load_and_process_data(filename: str, max_workers: int) -> Optional[MetricsData]:
    # Data structure of each log row
    log_row_dtype = np.dtype([
        ('timestamp_ms', 'u8'),
        ('global_allocated_paths', 'u8'),
        ('workers', worker_dtype, (max_workers,))
    ])

    # Load data from file
    try:
        data = np.memmap(filename, dtype=log_row_dtype, mode='r')
    except FileNotFoundError:
        print(f"Error: File '{filename}' not found.")
        return None
    except Exception as e:
        print(f"Error reading file: {e}")
        return None

    # Filter out rows with timestamp 0
    valid_rows = data[data['timestamp_ms'] > 0]
    if len(valid_rows) == 0:
        print("No valid data found")
        return None

    # Convert timestamps to seconds
    timestamps = valid_rows['timestamp_ms'] / 1000.0

    total_early = np.sum(valid_rows['workers']['early_backtracks'], axis=1)
    total_self = np.sum(valid_rows['workers']['self_consumed'], axis=1)
    avg_q_max = np.mean(valid_rows['workers']['max_queue_len'], axis=1)
    
    rate_early = np.diff(total_early, prepend=0)
    rate_self = np.diff(total_self, prepend=0)

    return {
        "timestamps": timestamps,
        "rate_early": rate_early,
        "rate_self": rate_self,
        "avg_q_max": avg_q_max
    }

def plot_data(metrics: MetricsData, max_workers: int) -> None:
    """
    Generates and displays the Plotly charts based on processed metrics.
    """
    
    timestamps = metrics["timestamps"]
    rate_early = metrics["rate_early"]
    rate_self = metrics["rate_self"]
    avg_q_max = metrics["avg_q_max"]

    fig = make_subplots(
        rows=2, cols=1,
        shared_xaxes=True,
        vertical_spacing=0.1,
        subplot_titles=("Parallel Efficiency (Events per Tick)", "System Load: Average Queue Length")
    )

    # --- Plot 1: Efficiency ---
    fig.add_trace(
        go.Scatter(x=timestamps, y=rate_early, mode='lines', name='Early Backtracks (Profit)',
                   line=dict(color='green')),
        row=1, col=1
    )
    fig.add_trace(
        go.Scatter(x=timestamps, y=rate_self, mode='lines', name='Self Consumed (Waste)',
                   line=dict(color='orange')),
        row=1, col=1
    )

    # --- Plot 2: Queue Health ---
    fig.add_trace(
        go.Scatter(x=timestamps, y=avg_q_max, mode='lines', name='Avg Max Queue Len',
                   line=dict(color='purple')),
        row=2, col=1
    )

    fig.update_layout(
        title=dict(text=f"Solver Metrics ({max_workers} Workers)"),
        hovermode="x unified",
        legend=dict(orientation="h", yanchor="bottom", y=1.05)
    )
    fig.update_xaxes(title_text="Time (s)", row=2, col=1)
    fig.show()

# --- CLI ---
parser = argparse.ArgumentParser(description="Visualize solver metrics from binary logs.")
parser.add_argument("filename", nargs="?", default="metrics.bin", help="Path to binary file")
parser.add_argument("--workers", "-w", type=int, default=16, help="Max workers")
args = parser.parse_args()

if __name__ == "__main__":
    metrics = load_and_process_data(args.filename, args.workers)
    if metrics:
        plot_data(metrics, args.workers)