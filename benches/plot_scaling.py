#!/usr/bin/env python3
"""
Plot speedup scaling from scaling_results.csv.

Usage:
    python3 benches/plot_scaling.py [scaling_results.csv]

Writes scaling_speedup.png to the current directory.
"""

import csv
import sys
from pathlib import Path
from collections import defaultdict

try:
    import matplotlib.pyplot as plt
    import matplotlib.ticker as ticker
except ImportError:
    sys.exit("matplotlib not found: pip install matplotlib")

csv_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("scaling_results.csv")
if not csv_path.exists():
    sys.exit(f"{csv_path} not found. Run:\n  cargo run --release --example parallel_scaling")

data: dict[str, dict[int, float]] = defaultdict(dict)  # mode -> threads -> speedup
mode_order: list[str] = []

with open(csv_path) as f:
    for row in csv.DictReader(f):
        t = int(row["threads"])
        mode = row["mode"]
        data[mode][t] = float(row["speedup"])
        if mode not in mode_order:
            mode_order.append(mode)

thread_counts = sorted(next(iter(data.values())).keys())
ideal = [float(t) for t in thread_counts]

# Style map — well-separated colours, dashed for reference modes
STYLES: dict[str, dict] = {
    "in-process":          {"color": "#888888", "marker": "o", "ls": "--",  "lw": 1.5, "zorder": 2},
    "Renderer shared Arc": {"color": "#2563eb", "marker": "s", "ls": "-",  "lw": 2.0, "zorder": 3},
    "Renderer per-thread": {"color": "#16a34a", "marker": "^", "ls": "-",  "lw": 2.5, "zorder": 4},
    "subprocess":          {"color": "#e05c2a", "marker": "D", "ls": ":",  "lw": 1.5, "zorder": 2},
}

def style(mode: str, key: str, default):
    return STYLES.get(mode, {}).get(key, default)

fig, axes = plt.subplots(1, 2, figsize=(14, 5))

# ── Left: speedup ──────────────────────────────────────────────────────────
ax = axes[0]
ax.plot(thread_counts, ideal, "k--", linewidth=1.2, alpha=0.5, label="Ideal (linear)", zorder=1)

for mode in mode_order:
    speedups = data[mode]
    ys = [speedups[t] for t in thread_counts]
    ax.plot(
        thread_counts, ys,
        color=style(mode, "color", "gray"),
        marker=style(mode, "marker", "o"),
        linestyle=style(mode, "ls", "-"),
        linewidth=style(mode, "lw", 1.5),
        zorder=style(mode, "zorder", 2),
        markersize=7, label=mode,
    )

# Annotate only the two Renderer modes to keep the chart readable
for mode in ("Renderer per-thread", "Renderer shared Arc"):
    if mode not in data:
        continue
    for t, y in zip(thread_counts, [data[mode][tc] for tc in thread_counts]):
        ax.annotate(
            f"{y:.1f}×", (t, y),
            textcoords="offset points", xytext=(6, 3),
            fontsize=7.5, color=style(mode, "color", "gray"),
        )

ax.set_xlabel("Worker count")
ax.set_ylabel("Speedup (×)")
ax.set_title("Speedup scaling by rendering mode")
ax.legend(fontsize=8.5)
ax.grid(True, alpha=0.25)
ax.xaxis.set_major_locator(ticker.FixedLocator(thread_counts))
ax.set_xlim(0.5, max(thread_counts) + 0.5)
ax.set_ylim(bottom=0)

# ── Right: absolute wall time ───────────────────────────────────────────────
ax2 = axes[1]

# Re-read mean_ms from CSV
mean_ms: dict[str, dict[int, float]] = defaultdict(dict)
with open(csv_path) as f:
    for row in csv.DictReader(f):
        mean_ms[row["mode"]][int(row["threads"])] = float(row["mean_ms"])

for mode in mode_order:
    ys = [mean_ms[mode][t] for t in thread_counts]
    ax2.plot(
        thread_counts, ys,
        color=style(mode, "color", "gray"),
        marker=style(mode, "marker", "o"),
        linestyle=style(mode, "ls", "-"),
        linewidth=style(mode, "lw", 1.5),
        zorder=style(mode, "zorder", 2),
        markersize=7, label=mode,
    )

ax2.set_xlabel("Worker count")
ax2.set_ylabel("Wall time for 16 tasks (ms)")
ax2.set_title("Absolute wall time (lower is better)")
ax2.legend(fontsize=8.5)
ax2.grid(True, alpha=0.25)
ax2.xaxis.set_major_locator(ticker.FixedLocator(thread_counts))
ax2.set_xlim(0.5, max(thread_counts) + 0.5)
ax2.set_ylim(bottom=0)

fig.suptitle(
    "PNG rendering: original render() vs Renderer (shared Arc) vs Renderer (per-thread clone)\n"
    "medium.html · 800×600 · 16 tasks per sample",
    fontsize=9.5,
)
fig.tight_layout()

out = Path("scaling_speedup.png")
fig.savefig(out, dpi=150)
print(f"Saved {out.resolve()}")

# ── Console table ───────────────────────────────────────────────────────────
col = 22
print(f"\n{'Threads':>8}  {'Mode':<{col}}  {'Mean(ms)':>9}  {'Speedup':>8}  {'Effic%':>7}")
print("-" * (8 + 2 + col + 2 + 9 + 2 + 8 + 2 + 7))
for t in thread_counts:
    for mode in mode_order:
        sp  = data[mode][t]
        ms  = mean_ms[mode][t]
        eff = sp / t * 100
        print(f"{t:>8}  {mode:<{col}}  {ms:>9.1f}  {sp:>8.2f}x  {eff:>6.1f}%")
