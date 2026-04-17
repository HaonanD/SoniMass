# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Cicada** (package name: `cicada`) is a Rust CLI tool that converts centroided mass spectrometry (mzML) data into audio (WAV). Each ion signal trace is rendered as a sine wave: m/z → frequency, intensity over time → amplitude envelope. The guiding principle is **full fidelity** — no filtering, no deconvolution, all signals rendered.

## Common Commands

```bash
# Build
cargo build
cargo build --release

# Run all tests
cargo test

# Run a single test by name
cargo test test_visualize_hill_building
cargo test test_pchip_interpolation

# Run the binary against an mzML file
./target/debug/cicada input.mzML --output my_track --mode dda
./target/debug/cicada input.mzML --output my_track --mode dia --ppm 10 --min-len 5 --speed 60
```

### CLI flags
| Flag | Default | Meaning |
|------|---------|---------|
| `--mode` | `dia` | `dda` (MS1 only) or `dia` (MS1 + MS2 dual output) |
| `--ppm` | `10.0` | m/z matching tolerance for hill building |
| `--min-len` | `5` | Minimum scan points for a hill to be kept |
| `--speed` | `1.0` | Time compression factor (e.g. `60.0` = 60 min → 1 min) |
| `--mslevel` | `all` | Filter input to `1`, `2`, or `all` |
| `--no-export-hills` | off | Skip exporting hill data to CSV (by default, hills are exported) |

## Architecture

The pipeline is strictly linear: **Parse → Hill Build → Synthesize → Write**.

```
mzML file
  └─ MzmlReader (mzdata crate) → Iterator<Spectrum>
       └─ HillBuilder → Vec<Hill>
            └─ Synthesizer (rayon parallel) → Vec<f32>
                 └─ AudioWriter (hound crate) → .wav
```

### Core data types (`src/core/structs.rs`)
- `Peak { mz: f64, intensity: f32 }` — one centroided mass peak
- `Spectrum { index, time, ms_level, peaks: Vec<Peak> }` — one scan; `time` is stored in **seconds** (reader converts from minutes)
- `Hill` — a sparse ion trace: parallel `Vec<scan_indices>`, `Vec<times>`, `Vec<intensity_values>`. Maintains `mz_guess` (rolling average for matching) and `average_mz` (intensity-weighted, used for frequency).

### Hill Building (`src/algo/hill_builder.rs`)
Implements Dinosaur-style **two-pointer greedy matching** in `O(N)` per scan:
- Active hills and incoming peaks are both sorted by m/z.
- A match within `ppm_tolerance` extends the hill; a miss advances the hill pointer (gap tracking); a new peak starts a fresh hill.
- Gap skipping: a hill stays active for up to `max_gap` consecutive unmatched scans (default 1).
- **No splitting, no isotope scoring** — intentional divergence from Dinosaur.
- Peaks in each `Spectrum` **must be m/z-sorted** for the greedy loop to be correct. The mzdata library delivers them sorted.

### Synthesis (`src/synth/`)
- **`PchipInterpolator`** (`interpolate.rs`): shape-preserving cubic Hermite interpolation on the sparse `(time, intensity)` points of a Hill. Guarantees non-negative output and avoids overshoot at peak edges.
- **`Oscillator::render_into_chunk`** (`oscillator.rs`): writes a Hill's audio contribution directly into a `&mut [f32]` chunk slice (no intermediate Vec allocation). Frequency mapped by forward-linear: `[300, 1000] m/z → [30, 4200] Hz`. Two inner-loop optimizations:
  - **Amplitude linear interpolation**: PCHIP is called every `AMP_INTERP_STEP = 64` samples; amplitude linearly interpolated between calls, reducing PCHIP evaluations ~64×. The constant `AMP_INTERP_STEP` is the sole knob controlling interpolation density.
  - **sin recurrence**: phase is advanced via `sin(θ+Δθ) = sinθ·cosΔθ + cosθ·sinΔθ` — only one `sin`/`cos` call per 64-sample block; inner loop is pure multiply-add.
  - Mixing (summation of all Hills) is the `+=` in this function — no separate mixer struct exists.
- **`Synthesizer::render`** (`synthesizer.rs`): (1) pre-builds all `PchipInterpolator` objects in parallel — each constructed exactly once per Hill; (2) partitions Hills into 1-second temporal buckets (stored as index lists); (3) renders buckets in parallel via `rayon`, writing directly into the output buffer. Final buffer peak-normalized to 0.9.

### Hill Export (`src/io/hill_writer.rs`)
By default, after hill building and time normalization, hills are written to CSV alongside the WAV output:
- `{output}_ms1_hills.csv` — MS1 hills
- `{output}_ms2_hills.csv` — MS2 hills (DIA mode only)

CSV format: `id,average_mz,time,intensity` — one row per data point. Times are already normalized (start at 0) and speed-scaled, matching the audio timeline. Use `--no-export-hills` to suppress this output.

A companion Python script `tools/plot_hills_3d.py` reads a hill CSV and produces a static 3D visualization (time × m/z × intensity) with dual m/z/Hz axis labels, saved as a PNG alongside the CSV.

### DDA vs DIA in `main.rs`
- **DDA**: only MS1 spectra are collected → one `_ms1.wav` output.
- **DIA**: MS1 → `_ms1.wav`; MS2 treated as a pseudo-continuous time series → `_ms2.wav`. Both use the identical hill-building + synthesis pipeline.

## Key Design Constraints

- Input **must** be centroided mzML. Profile-mode data will produce wrong results.
- `Hill.times` are normalized to start at 0 and divided by `speed` in `main.rs` **before** being passed to the synthesizer. Never store absolute retention times inside a Hill after normalization.
- The synthesizer expects `Hill.times` to be strictly increasing (guaranteed by scan order).
- `AudioWriter` writes 32-bit float WAV (not 16-bit PCM). Samples must be in `[-1.0, 1.0]` — the synthesizer's normalize step ensures this.
