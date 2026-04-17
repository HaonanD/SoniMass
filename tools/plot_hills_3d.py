"""
plot_hills_3d.py - Static 3D visualization of hill data from a CSV file.

Usage:
    python plot_hills_3d.py <path_to_hill_csv>

The CSV must have columns: id, average_mz, time, intensity
Output: a PNG file with the same base name as the input CSV.
"""

import sys
import os
from pathlib import Path

import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")  # non-interactive backend for file output
import matplotlib.pyplot as plt
from mpl_toolkits.mplot3d import Axes3D  # noqa: F401 (registers 3D projection)
from matplotlib.cm import ScalarMappable
from matplotlib.colors import Normalize


# ---------------------------------------------------------------------------
# m/z <-> Hz conversion (linear map: [300, 1000] mz -> [30, 4200] Hz)
# ---------------------------------------------------------------------------
MZ_LOW, MZ_HIGH = 300.0, 1000.0
HZ_LOW, HZ_HIGH = 30.0, 4200.0

def mz_to_hz(mz):
    return HZ_LOW + (mz - MZ_LOW) / (MZ_HIGH - MZ_LOW) * (HZ_HIGH - HZ_LOW)

def hz_to_mz(hz):
    return MZ_LOW + (hz - HZ_LOW) / (HZ_HIGH - HZ_LOW) * (MZ_HIGH - MZ_LOW)


def main():
    if len(sys.argv) < 2:
        print("Usage: python plot_hills_3d.py <hill_csv_file>")
        sys.exit(1)

    csv_path = Path(sys.argv[1])
    if not csv_path.exists():
        print(f"Error: file not found: {csv_path}")
        sys.exit(1)

    output_path = csv_path.with_suffix(".png")

    # ------------------------------------------------------------------
    # Read data
    # ------------------------------------------------------------------
    print(f"Reading data from: {csv_path}")
    df = pd.read_csv(csv_path)

    required_cols = {"id", "average_mz", "time", "intensity"}
    missing = required_cols - set(df.columns)
    if missing:
        print(f"Error: CSV is missing columns: {missing}")
        sys.exit(1)

    hill_ids = df["id"].unique()
    n_hills = len(hill_ids)
    print(f"Plotting {n_hills} hills...")

    # ------------------------------------------------------------------
    # Color normalization across all m/z values
    # ------------------------------------------------------------------
    mz_min = df["average_mz"].min()
    mz_max = df["average_mz"].max()
    # Guard against degenerate case where all hills have the same m/z
    if mz_min == mz_max:
        mz_min -= 1.0
        mz_max += 1.0

    norm = Normalize(vmin=mz_min, vmax=mz_max)
    cmap = plt.get_cmap("plasma")

    # ------------------------------------------------------------------
    # Build figure
    # ------------------------------------------------------------------
    fig = plt.figure(figsize=(16, 10), dpi=150)
    ax = fig.add_subplot(111, projection="3d")

    # Group by hill id and draw each hill as a single 3D line
    grouped = df.groupby("id", sort=False)
    for hill_id, group in grouped:
        group_sorted = group.sort_values("time")
        xs = group_sorted["time"].to_numpy()
        mz_val = group_sorted["average_mz"].iloc[0]
        ys = np.full(len(xs), mz_val)
        zs = group_sorted["intensity"].to_numpy()
        color = cmap(norm(mz_val))
        ax.plot(xs, ys, zs, color=color, linewidth=0.8, alpha=0.9)

    # ------------------------------------------------------------------
    # Axis labels and title
    # ------------------------------------------------------------------
    ax.set_xlabel("Time (s)", labelpad=10)
    ax.set_ylabel("m/z", labelpad=10)
    ax.set_zlabel("Intensity", labelpad=10)
    ax.set_title(f"Hill Map: {csv_path.name}", pad=15, fontsize=13)

    # ------------------------------------------------------------------
    # Secondary Hz tick labels on the Y (m/z) axis
    # ------------------------------------------------------------------
    # Choose evenly spaced Hz values that fall within the plotted m/z range
    hz_candidates = np.array([500, 1000, 1500, 2000, 2500, 3000, 3500, 4000], dtype=float)
    # Convert to m/z and keep only those inside [mz_min, mz_max]
    mz_candidates = hz_to_mz(hz_candidates)
    mask = (mz_candidates >= mz_min) & (mz_candidates <= mz_max)
    hz_ticks_hz = hz_candidates[mask]
    hz_ticks_mz = mz_candidates[mask]

    if len(hz_ticks_mz) > 0:
        # Get current primary m/z ticks so we can build a combined tick list
        current_yticks = list(ax.get_yticks())
        # Filter primary ticks to those in range (matplotlib may include out-of-range ticks)
        current_yticks = [t for t in current_yticks if mz_min <= t <= mz_max]

        # Merge primary + secondary tick positions, deduplicate, sort
        all_tick_positions = sorted(set(list(hz_ticks_mz) + current_yticks))

        # Build labels: for positions that correspond to an Hz secondary tick, show
        # both m/z and Hz; for primary-only ticks show just the m/z value.
        hz_pos_set = {round(p, 4): hz for p, hz in zip(hz_ticks_mz, hz_ticks_hz)}
        labels = []
        for pos in all_tick_positions:
            key = round(pos, 4)
            if key in hz_pos_set:
                labels.append(f"{pos:.0f}\n({hz_pos_set[key]:.0f} Hz)")
            else:
                labels.append(f"{pos:.0f}")

        ax.set_yticks(all_tick_positions)
        ax.set_yticklabels(labels, fontsize=7)

    # ------------------------------------------------------------------
    # Colorbar
    # ------------------------------------------------------------------
    sm = ScalarMappable(cmap=cmap, norm=norm)
    sm.set_array([])
    cbar = fig.colorbar(sm, ax=ax, shrink=0.5, aspect=15, pad=0.1)
    cbar.set_label("m/z", fontsize=10)

    # ------------------------------------------------------------------
    # Viewing angle
    # ------------------------------------------------------------------
    ax.view_init(elev=20, azim=-60)

    # ------------------------------------------------------------------
    # Save
    # ------------------------------------------------------------------
    plt.tight_layout()
    print(f"Saving to: {output_path}")
    plt.savefig(output_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print("Done.")


if __name__ == "__main__":
    main()
