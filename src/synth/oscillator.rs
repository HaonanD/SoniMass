use crate::core::config::FrequencyConfig;
use crate::core::structs::Hill;
use crate::synth::interpolate::PchipInterpolator;
use crate::synth::mapping;
use std::f32::consts::PI;

/// Number of audio samples between PCHIP amplitude evaluations.
/// Amplitude is linearly interpolated between evaluations.
/// 64 samples ≈ 1.45ms at 44100 Hz — far above the envelope's variation frequency
/// (determined by MS scan rate, typically 5–20 Hz).
const AMP_INTERP_STEP: usize = 64;

pub struct Oscillator;

impl Oscillator {
    /// Renders a hill's audio contribution directly into the provided chunk buffer.
    ///
    /// Optimizations vs. the previous generate_window approach:
    /// 1. `pchip` is pre-built externally (not reconstructed here).
    /// 2. Writes directly into `chunk` — no intermediate Vec allocation.
    /// 3. Amplitude is sampled via PCHIP every AMP_INTERP_STEP samples and
    ///    linearly interpolated in between, reducing PCHIP calls by ~64×.
    /// 4. sin() is computed via recurrence — only one sin/cos call per
    ///    AMP_INTERP_STEP block; inner loop uses multiply-add only.
    pub fn render_into_chunk(
        hill: &Hill,
        pchip: &PchipInterpolator,
        sample_rate: u32,
        chunk: &mut [f32],
        chunk_start_idx: usize,
        freq_cfg: &FrequencyConfig,
        mapping_method: u32,
    ) {
        let f = mapping::map_frequency(mapping_method, hill.average_mz, freq_cfg);

        let hill_start_time = *hill.times.first().unwrap_or(&0.0);
        let hill_end_time = *hill.times.last().unwrap_or(&0.0);
        let chunk_end_idx = chunk_start_idx + chunk.len();
        let chunk_start_time = chunk_start_idx as f64 / sample_rate as f64;
        let chunk_end_time = chunk_end_idx as f64 / sample_rate as f64;

        let actual_start_time = chunk_start_time.max(hill_start_time);
        let actual_end_time = chunk_end_time.min(hill_end_time);

        if actual_start_time >= actual_end_time {
            return;
        }

        let start_sample = (actual_start_time * sample_rate as f64).floor() as usize;
        let end_sample = (actual_end_time * sample_rate as f64).ceil() as usize;

        // Precompute sin recurrence deltas — constant for this hill's frequency.
        let delta_theta = 2.0 * PI * f / sample_rate as f32;
        let delta_sin = delta_theta.sin();
        let delta_cos = delta_theta.cos();

        // Process in AMP_INTERP_STEP blocks.
        let mut block_start = start_sample;
        while block_start < end_sample {
            let block_end = (block_start + AMP_INTERP_STEP).min(end_sample);
            let block_len = block_end - block_start;

            // PCHIP evaluated only at block boundaries (2 calls per block).
            let t0 = block_start as f64 / sample_rate as f64;
            let t1 = block_end as f64 / sample_rate as f64;
            let amp_start = pchip.get_value(t0);
            let amp_end = pchip.get_value(t1);
            let amp_step = (amp_end - amp_start) / block_len as f32;

            // Initialize sin recurrence exactly at t0 to prevent phase drift
            // across block boundaries.
            let theta0 = 2.0 * PI * f * t0 as f32;
            let mut sin_v = theta0.sin();
            let mut cos_v = theta0.cos();
            let mut amp = amp_start;

            for i in 0..block_len {
                let chunk_local_idx = (block_start + i) - chunk_start_idx;
                chunk[chunk_local_idx] += amp * sin_v;

                // Advance phase via recurrence: no sin() call needed.
                let new_sin = sin_v * delta_cos + cos_v * delta_sin;
                cos_v = cos_v * delta_cos - sin_v * delta_sin;
                sin_v = new_sin;
                amp += amp_step;
            }

            block_start = block_end;
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::structs::{Hill, Peak};

    #[test]
    fn test_render_into_chunk() {
        let mut hill = Hill::new(0, Peak { mz: 650.0, intensity: 1.0 }, 0, 0.0);
        hill.push(Peak { mz: 650.0, intensity: 1.0 }, 1, 1.0);

        let pchip = PchipInterpolator::new(&hill.times, &hill.intensity_values);
        let sample_rate = 44100u32;
        let mut chunk = vec![0.0f32; sample_rate as usize];

        Oscillator::render_into_chunk(
            &hill, &pchip, sample_rate, &mut chunk, 0, &FrequencyConfig::default(), 1,
        );

        let has_signal = chunk.iter().any(|&s| s.abs() > 0.0);
        assert!(has_signal);
    }
}
