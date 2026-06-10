#!/usr/bin/env python3
"""
Plot ideal vs actual speedup for the parallel_throughput benchmark.

Usage:
    python3 benches/plot_speedup.py

Reads Criterion JSON from target/criterion/parallel_throughput/
and writes speedup.png to the current directory.

Dependencies: pip install matplotlib
"""

import json
import os
import sys
from pathlib import Path

try:
    import matplotlib.pyplot as plt
    import matplotlib.ticker as ticker
except ImportError:
    sys.exit("matplotlib not found. Install it with:  pip install matplotlib")

CRITERION_DIR = Path("target/criterion/parallel_throughput")


def load_mean_ns(bench_name: str) -> float | None:
    """Return the mean estimate in nanoseconds for a benchmark, or None if missing."""
    estimates_path = CRITERION_DIR / bench_name / "new" / "estimates.json"
    if not estimates_path.exists():
        # Try without the group sub-directory (Criterion 0.5 layout)
        estimates_path = CRITERION_DIR / bench_name / "estimates.json"
    if not estimates_path.exists():
        return None
    with open(estimates_path) as f:
        data = json.load(f)
    return data["mean"]["point_estimate"]  # nanoseconds


def collect_results() -> dict[int, float]:
    """
    Return a dict mapping thread_count -> mean_time_ns.

    Criterion 0.5 layout:
      target/criterion/parallel_throughput/threads/<N>/new/estimates.json
      target/criterion/parallel_throughput/threads_1_sequential/new/estimates.json
    """
    if not CRITERION_DIR.exists():
        sys.exit(
            f"Criterion output directory not found: {CRITERION_DIR}\n"
            "Run the benchmark first:\n"
            "  cargo bench --bench parallel_throughput"
        )

    results: dict[int, float] = {}

    # Parallel variants (threads/<N>)
    threads_dir = CRITERION_DIR / "threads"
    if threads_dir.exists():
        for param_dir in sorted(threads_dir.iterdir()):
            try:
                t = int(param_dir.name)
            except ValueError:
                continue
            est = load_mean_ns(f"threads/{param_dir.name}")
            if est is not None:
                results[t] = est

    # Prefer the explicit sequential baseline for thread count 1 when available,
    # as it avoids thread-spawn overhead in the measurement.
    seq_ns = load_mean_ns("threads_1_sequential")
    if seq_ns is not None:
        results[1] = seq_ns

    return results


def main() -> None:
    results = collect_results()

    if not results:
        sys.exit(
            "No benchmark results found.\n"
            "Run the benchmark first:\n"
            "  cargo bench --bench parallel_throughput"
        )

    if 1 not in results:
        sys.exit("Sequential baseline (threads=1) result not found.")

    baseline_ns = results[1]
    thread_counts = sorted(results.keys())

    actual_speedup = [baseline_ns / results[t] for t in thread_counts]
    ideal_speedup = [float(t) for t in thread_counts]

    fig, ax = plt.subplots(figsize=(8, 5))

    ax.plot(thread_counts, ideal_speedup, "k--", linewidth=1.5, label="Ideal (linear)")
    ax.plot(thread_counts, actual_speedup, "o-", color="#4f46e5", linewidth=2,
            markersize=7, label="Actual speedup")

    # Annotate efficiency
    for t, a in zip(thread_counts, actual_speedup):
        eff = a / t * 100
        ax.annotate(f"{eff:.0f}%", (t, a), textcoords="offset points",
                    xytext=(6, 4), fontsize=8, color="#666666")

    ax.set_xlabel("Thread count")
    ax.set_ylabel("Speedup (×)")
    ax.set_title("PNG rendering throughput: ideal vs actual speedup\n"
                 f"(medium.html, 800×600, {16} tasks per sample)")
    ax.legend()
    ax.grid(True, alpha=0.3)
    ax.xaxis.set_major_locator(ticker.MaxNLocator(integer=True))

    max_threads = max(thread_counts)
    ax.set_xlim(0.5, max_threads + 0.5)
    ax.set_ylim(bottom=0)

    out = Path("speedup.png")
    fig.tight_layout()
    fig.savefig(out, dpi=150)
    print(f"Saved {out.resolve()}")

    # Also print a table
    print(f"\n{'Threads':>8}  {'Mean (ms)':>10}  {'Speedup':>8}  {'Efficiency':>10}")
    print("-" * 44)
    for t in thread_counts:
        ms = results[t] / 1e6
        sp = baseline_ns / results[t]
        eff = sp / t * 100
        print(f"{t:>8}  {ms:>10.1f}  {sp:>8.2f}x  {eff:>9.1f}%")


if __name__ == "__main__":
    main()
