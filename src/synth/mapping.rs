//! Centralized signal-mapping module.
//!
//! All mapping_method variants live here.  To add a new method:
//!   1. Add a branch to `map_frequency` and/or `transform_intensity`.
//!   2. Raise the upper-bound check in `main.rs`.
//!   3. Document it in `mapping_method.txt`.

use crate::core::config::FrequencyConfig;

// ── Method 3: full audible spectrum ─────────────────────────────────────────
const AUDIBLE_MIN_HZ: f64 = 20.0;
const AUDIBLE_MAX_HZ: f64 = 20_000.0;

// ── Methods 4 & 5: musical quantization ─────────────────────────────────────
/// Reference pitch: A4 = 440 Hz
const REF_FREQ: f32 = 440.0;

/// Major pentatonic scale — semitone offsets within one octave (0 = root A).
/// Degrees: A(0) B(2) C#(4) E(7) F#(9)
const PENTATONIC: [i32; 5] = [0, 2, 4, 7, 9];

// ── Public API ───────────────────────────────────────────────────────────────

/// Map an m/z value to an audio frequency (Hz) according to `method`.
///
/// | Method | Mapping |
/// |--------|---------|
/// | 1 (default) | Logarithmic, config range |
/// | 2 | Linear, config range |
/// | 3 | Logarithmic, full audible range [20, 20 000] Hz |
/// | 4 | Logarithmic → snapped to major-pentatonic scale |
/// | 5 | Logarithmic → snapped to 12-TET chromatic scale |
/// | 6 | Logarithmic (freq identical to method 1; amplitude differs) |
/// | 7 | Logarithmic (freq identical to method 1; amplitude differs) |
/// | 8 | Inverted logarithmic (high m/z → low frequency) |
pub fn map_frequency(method: u32, mz: f64, cfg: &FrequencyConfig) -> f32 {
    match method {
        2 => linear(mz, cfg, cfg.min_freq, cfg.max_freq),
        3 => log(mz, cfg, AUDIBLE_MIN_HZ, AUDIBLE_MAX_HZ),
        4 => snap_to_scale(log(mz, cfg, cfg.min_freq, cfg.max_freq), &PENTATONIC),
        5 => snap_to_chromatic(log(mz, cfg, cfg.min_freq, cfg.max_freq)),
        8 => log_inverted(mz, cfg),
        _ => log(mz, cfg, cfg.min_freq, cfg.max_freq), // 1, 6, 7
    }
}

/// Transform a raw intensity value to an amplitude scalar according to `method`.
///
/// | Method | Transform |
/// |--------|-----------|
/// | 6 | Linear (no compression) |
/// | 7 | Square-root compression |
/// | _ | ln(log_offset + I) — default logarithmic compression |
pub fn transform_intensity(method: u32, intensity: f32, log_offset: f32) -> f32 {
    match method {
        6 => intensity,                        // linear — loud peaks dominate
        7 => intensity.max(0.0).sqrt(),        // sqrt — gentle compression
        _ => (log_offset + intensity).ln(),    // ln(1+I) — default
    }
}

// ── Private helpers ──────────────────────────────────────────────────────────

/// Logarithmic (perceptually uniform) m/z → frequency mapping.
/// Equal m/z intervals span equal musical octaves.
fn log(mz: f64, cfg: &FrequencyConfig, fmin: f64, fmax: f64) -> f32 {
    let t = t_from_mz(mz, cfg);
    (fmin.ln() + t * (fmax.ln() - fmin.ln())).exp() as f32
}

/// Linear m/z → frequency mapping.
/// Equal m/z intervals span equal Hz intervals.
fn linear(mz: f64, cfg: &FrequencyConfig, fmin: f64, fmax: f64) -> f32 {
    let t = t_from_mz(mz, cfg);
    (fmin + t * (fmax - fmin)) as f32
}

/// Inverted logarithmic mapping: high m/z → low frequency, low m/z → high frequency.
fn log_inverted(mz: f64, cfg: &FrequencyConfig) -> f32 {
    let t = 1.0 - t_from_mz(mz, cfg);
    (cfg.min_freq.ln() + t * (cfg.max_freq.ln() - cfg.min_freq.ln())).exp() as f32
}

/// Snap a frequency (Hz) to the nearest pitch on the 12-TET chromatic scale,
/// relative to A4 = 440 Hz.
fn snap_to_chromatic(freq: f32) -> f32 {
    if freq <= 0.0 {
        return REF_FREQ;
    }
    let semis = 12.0 * (freq / REF_FREQ).log2();
    let n = semis.round() as i32;
    REF_FREQ * 2.0f32.powf(n as f32 / 12.0)
}

/// Snap a frequency (Hz) to the nearest pitch of the given scale (expressed as
/// semitone classes within one octave, relative to A4 = 440 Hz).
fn snap_to_scale(freq: f32, classes: &[i32]) -> f32 {
    if freq <= 0.0 || classes.is_empty() {
        return REF_FREQ;
    }
    let semis = 12.0 * (freq / REF_FREQ).log2();
    let center = semis.round() as i32;
    let mut best_n = center;
    let mut best_dist = f32::MAX;
    // Search ±12 semitones around the continuous value; that window always
    // contains at least one note from any 5-or-more-note scale.
    for n in (center - 12)..=(center + 12) {
        let cls = n.rem_euclid(12) as i32;
        if classes.contains(&cls) {
            let dist = (semis - n as f32).abs();
            if dist < best_dist {
                best_dist = dist;
                best_n = n;
            }
        }
    }
    REF_FREQ * 2.0f32.powf(best_n as f32 / 12.0)
}

/// Normalize mz into [0, 1] after clamping to [cfg.min_mz, cfg.max_mz].
#[inline]
fn t_from_mz(mz: f64, cfg: &FrequencyConfig) -> f64 {
    let clamped = mz.max(cfg.min_mz).min(cfg.max_mz);
    (clamped - cfg.min_mz) / (cfg.max_mz - cfg.min_mz)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::FrequencyConfig;

    fn cfg() -> FrequencyConfig {
        FrequencyConfig::default() // min_mz=300, max_mz=1000, min_freq=30, max_freq=4200
    }

    // ── Method 1 (log, default) ──────────────────────────────────────────────
    #[test]
    fn test_method1_boundaries() {
        let c = cfg();
        assert!((map_frequency(1, 300.0, &c) - 30.0).abs() < 0.01);
        assert!((map_frequency(1, 1000.0, &c) - 4200.0).abs() < 1.0);
    }

    #[test]
    fn test_method1_midpoint_geometric_mean() {
        let c = cfg();
        let freq = map_frequency(1, 650.0, &c);
        let expected = (30.0_f32 * 4200.0_f32).sqrt();
        assert!((freq - expected).abs() < 1.0, "expected ~{expected:.1} Hz, got {freq:.1}");
    }

    // ── Method 2 (linear) ───────────────────────────────────────────────────
    #[test]
    fn test_method2_boundaries() {
        let c = cfg();
        assert!((map_frequency(2, 300.0, &c) - 30.0).abs() < 0.01);
        assert!((map_frequency(2, 1000.0, &c) - 4200.0).abs() < 1.0);
    }

    #[test]
    fn test_method2_midpoint_arithmetic_mean() {
        let c = cfg();
        let freq = map_frequency(2, 650.0, &c);
        assert!((freq - 2115.0).abs() < 1.0, "expected 2115 Hz, got {freq}");
    }

    // ── Method 3 (full audible range) ───────────────────────────────────────
    #[test]
    fn test_method3_boundaries() {
        let c = cfg();
        assert!((map_frequency(3, 300.0, &c) - 20.0).abs() < 0.01);
        assert!((map_frequency(3, 1000.0, &c) - 20_000.0).abs() < 1.0);
    }

    // ── Method 4 (pentatonic snap) ───────────────────────────────────────────
    #[test]
    fn test_method4_snaps_to_pentatonic() {
        // A4 = 440 Hz is already on the scale (class 0), should snap to itself
        let freq = snap_to_scale(440.0, &PENTATONIC);
        assert!((freq - 440.0).abs() < 0.5, "A4 should snap to itself, got {freq}");
    }

    #[test]
    fn test_method4_result_on_pentatonic_grid() {
        let c = cfg();
        for mz in [350.0, 450.0, 600.0, 750.0, 900.0] {
            let freq = map_frequency(4, mz, &c);
            // Verify freq = REF * 2^(n/12) for some integer n that is in PENTATONIC mod 12
            let semis_from_ref = 12.0 * (freq / REF_FREQ).log2();
            let n = semis_from_ref.round() as i32;
            let cls = n.rem_euclid(12) as i32;
            assert!(
                PENTATONIC.contains(&cls),
                "mz={mz}: freq={freq:.2} Hz → semitone class {cls} not in pentatonic scale"
            );
        }
    }

    // ── Method 5 (chromatic snap) ────────────────────────────────────────────
    #[test]
    fn test_method5_a4_identity() {
        let freq = snap_to_chromatic(440.0);
        assert!((freq - 440.0).abs() < 0.5, "A4 should snap to itself, got {freq}");
    }

    #[test]
    fn test_method5_snaps_to_chromatic_grid() {
        for input in [350.0_f32, 500.0, 1000.0, 2000.0] {
            let freq = snap_to_chromatic(input);
            let semis = 12.0 * (freq / REF_FREQ).log2();
            let frac = semis - semis.round();
            assert!(
                frac.abs() < 0.01,
                "input={input}: snapped to {freq:.2} Hz, semitone offset {frac:.4} — not on 12-TET grid"
            );
        }
    }

    // ── Method 8 (inverted log) ───────────────────────────────────────────────
    #[test]
    fn test_method8_inverted() {
        let c = cfg();
        // Low m/z → high freq
        assert!((map_frequency(8, 300.0, &c) - 4200.0).abs() < 1.0);
        // High m/z → low freq
        assert!((map_frequency(8, 1000.0, &c) - 30.0).abs() < 0.01);
    }

    // ── transform_intensity ───────────────────────────────────────────────────
    #[test]
    fn test_transform_method6_linear() {
        assert_eq!(transform_intensity(6, 100.0, 1.0), 100.0);
        assert_eq!(transform_intensity(6, 0.0, 1.0), 0.0);
    }

    #[test]
    fn test_transform_method7_sqrt() {
        let v = transform_intensity(7, 100.0, 1.0);
        assert!((v - 10.0).abs() < 1e-4, "expected sqrt(100)=10, got {v}");
    }

    #[test]
    fn test_transform_default_log() {
        let v = transform_intensity(1, 0.0, 1.0);
        // ln(1+0) = 0
        assert!(v.abs() < 1e-4);
        let v2 = transform_intensity(1, 99.0, 1.0);
        let expected = (1.0_f32 + 99.0).ln();
        assert!((v2 - expected).abs() < 1e-4);
    }

    // ── mz clamping ───────────────────────────────────────────────────────────
    #[test]
    fn test_clamping_below() {
        let c = cfg();
        assert_eq!(map_frequency(1, 100.0, &c), map_frequency(1, 300.0, &c));
    }

    #[test]
    fn test_clamping_above() {
        let c = cfg();
        assert_eq!(map_frequency(1, 9999.0, &c), map_frequency(1, 1000.0, &c));
    }
}
