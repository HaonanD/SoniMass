# Cicada

> [中文版](README.zh.md)

Cicada is a CLI tool that converts centroided mass spectrometry data (mzML) into audio (WAV). Each ion trace is rendered as a sine wave: m/z determines frequency, and intensity over time forms the amplitude envelope.

> **On the name**: Cicadas produce one of nature's most spectacular displays of "massively concurrent" sound — thousands of individuals vibrating simultaneously to form a continuous, overwhelming chorus. Cicada renders the tens of thousands of ion signals in a mass spectrum as interwoven sine waves, giving silent data a voice.

## How It Works

A complex sound can be decomposed into a sum of sine waves, each with its own time-varying amplitude and frequency:

$$f(t) = \sum_{n=1}^{\infty} A_n(t) \cdot \sin(2\pi F_n t)$$

Cicada maps mass spectrometry data onto this model:

- **Frequency ($F_n$)**: mapped from **m/z**
- **Amplitude envelope ($A_n(t)$)**: the extracted ion chromatogram (XIC) for that m/z

## Signal Mapping

**m/z → Frequency**: Logarithmic mapping over `[300, 1000] m/z → [30, 4200] Hz`. Equal m/z intervals span equal octaves, matching human pitch perception.

$$F = \exp\bigl((1-t)\ln 30 + t\ln 4200\bigr), \quad t = \frac{m/z - 300}{1000 - 300}$$

**Intensity → Amplitude**: Log-compressed to bring 5–6 decades of MS dynamic range into an audible range.

$$A = \ln(1 + \text{intensity})$$

Each ion's intensity over time forms the amplitude envelope via PCHIP interpolation.

## Workflow

1. **Hill Building** — links m/z-adjacent peaks across consecutive scans into ion traces (Hills) using Dinosaur-style two-pointer greedy matching, O(N) per scan
2. **Frequency & envelope** — intensity-weighted average m/z sets the frequency; sparse `(time, intensity)` points form the envelope
3. **PCHIP interpolation** — generates a continuous, shape-preserving amplitude envelope aligned to the audio sample rate
4. **Synthesis** — sums all sine waves into the final waveform

### DDA vs DIA

- **DDA**: MS1 only → single `_ms1.wav` output
- **DIA**: MS1 → `_ms1.wav`; MS2 (continuously cycling) → `_ms2.wav`

## Visualization

Each run produces two companion files by default:

- **Heatmap PNG** (`*_heatmap.png`): time × m/z raster image with Plasma colormap
- **Interactive HTML viewer** (`*.html`): heatmap with axis labels and an embedded WAV player; a white playhead line tracks the current position during playback

## Installation

```bash
cargo build --release
```

Or download the pre-built binary from the [Releases](https://github.com/HaonanD/SoniMass/releases) page.

## Usage

```bash
# DIA mode (default)
./target/release/cicada input.mzML --output my_track

# DDA mode (MS1 only)
./target/release/cicada input.mzML --output my_track --mode dda
```

### CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--mode` | `dia` | `dda` or `dia` |
| `--ppm` | `10.0` | m/z matching tolerance for hill building |
| `--min-len` | `5` | Minimum scan points per hill |
| `--speed` | `1.0` | Time compression factor (e.g. `60.0` compresses 60 min → 1 min) |
| `--mslevel` | `all` | Filter input to `1`, `2`, or `all` |
| `--start` | — | Time range start, in minutes |
| `--width` | — | Time range length, in minutes |
| `--no-export-hills` | — | Skip Hill CSV export |
| `--no-export-viz` | — | Skip visualization output |

### Output Files

```
DIA:  <output>_ms1.wav, <output>_ms2.wav
DDA:  <output>_ms1.wav
Hills (default): <output>_ms1_hills.csv, <output>_ms2_hills.csv
Viz   (default): <output>_ms1_heatmap.png, <output>_ms1.html
                 <output>_ms2_heatmap.png, <output>_ms2.html  (DIA only)
```

## Prerequisites

Input must be **centroided** mzML. Profile-mode data should be converted first with msconvert or equivalent.

## License

[MIT](LICENSE)
