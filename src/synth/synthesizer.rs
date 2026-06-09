use crate::core::config::Config;
use crate::core::structs::Hill;
use crate::synth::interpolate::PchipInterpolator;
use crate::synth::mapping;
use crate::synth::oscillator::Oscillator;
use rayon::prelude::*;

pub struct Synthesizer {
    sample_rate: u32,
    config: Config,
    mapping_method: u32,
}

impl Synthesizer {
    pub fn new(sample_rate: u32, config: Config, mapping_method: u32) -> Self {
        Self { sample_rate, config, mapping_method }
    }

    pub fn render(&self, hills: Vec<Hill>) -> Vec<f32> {
        if hills.is_empty() {
            return vec![];
        }

        // 1. Find total audio duration
        let max_end_time = hills.par_iter()
            .map(|h| h.times.last().copied().unwrap_or(0.0))
            .reduce(|| 0.0, f64::max);

        let total_samples = (max_end_time * self.sample_rate as f64).ceil() as usize;
        if total_samples == 0 {
            return vec![];
        }

        println!("      Audio duration: {:.2} minutes", max_end_time / 60.0);
        println!("      Total samples to render: {}", total_samples);

        // 2. Pre-build all PCHIP interpolators in parallel — once per hill.
        //    Log-transform intensities (ln(log_offset + x)) to compress dynamic range:
        //    weak signals become more audible relative to dominant peaks.
        let log_offset = self.config.intensity.log_offset;
        let method = self.mapping_method;
        println!("      Pre-building {} PCHIP interpolators...", hills.len());
        let pchips: Vec<PchipInterpolator> = hills.par_iter()
            .map(|h| {
                let transformed: Vec<f32> = h.intensity_values.iter()
                    .map(|&v| mapping::transform_intensity(method, v, log_offset))
                    .collect();
                PchipInterpolator::new(&h.times, &transformed)
            })
            .collect();

        // 3. Spatial-Temporal Partitioning (Bucket Sort)
        //    Store hill indices rather than references so the render loop can
        //    borrow hills[] and pchips[] immutably across threads.
        let bucket_duration = 1.0_f64; // seconds
        let num_buckets = (max_end_time / bucket_duration).ceil() as usize + 1;

        println!("      Partitioning {} hills into {} temporal buckets...", hills.len(), num_buckets);

        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); num_buckets];

        for (i, hill) in hills.iter().enumerate() {
            let start_t = *hill.times.first().unwrap();
            let end_t = *hill.times.last().unwrap();
            let start_bucket = (start_t / bucket_duration).floor() as usize;
            let end_bucket = (end_t / bucket_duration).floor() as usize;
            for b in start_bucket..=end_bucket {
                if b < num_buckets {
                    buckets[b].push(i);
                }
            }
        }

        // 4. Initialize the global audio buffer
        let mut final_buffer = vec![0.0f32; total_samples];
        let chunk_size = (bucket_duration * self.sample_rate as f64) as usize;
        let freq_cfg = &self.config.frequency;
        let sample_rate = self.sample_rate;
        let mapping_method = self.mapping_method;

        println!("      Rendering buckets in parallel...");

        // 5. Render buckets in parallel.
        //    Each thread writes directly into its chunk slice — no intermediate
        //    Vec allocation per hill.  hills[] and pchips[] are read-only and
        //    are safely shared across threads (both are Sync).
        final_buffer.par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(bucket_idx, chunk)| {
                let chunk_start_idx = bucket_idx * chunk_size;

                if bucket_idx < buckets.len() {
                    for &hill_idx in &buckets[bucket_idx] {
                        Oscillator::render_into_chunk(
                            &hills[hill_idx],
                            &pchips[hill_idx],
                            sample_rate,
                            chunk,
                            chunk_start_idx,
                            freq_cfg,
                            mapping_method,
                        );
                    }
                }
            });

        // 6. Normalize
        println!("      Normalizing audio...");
        Self::normalize_to(&mut final_buffer, self.config.amplitude.normalize_target);

        final_buffer
    }

    fn normalize_to(buffer: &mut [f32], target: f32) {
        let max_amp = buffer.par_iter()
            .map(|&s| s.abs())
            .reduce(|| 0.0f32, f32::max);

        if max_amp > 1e-6 {
            let scale = target / max_amp;
            buffer.par_iter_mut().for_each(|s| *s *= scale);
        }
    }
}
