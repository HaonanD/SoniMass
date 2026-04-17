use crate::core::structs::Hill;
use crate::synth::oscillator::Oscillator;
use rayon::prelude::*;

pub struct Synthesizer {
    sample_rate: u32,
}

impl Synthesizer {
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    /// Highly optimized render for very long durations (60+ minutes) and millions of Hills.
    /// Uses Spatial-Temporal Partitioning (Bucket Sort) to achieve O(1) lookup during synthesis.
    pub fn render(&self, hills: Vec<Hill>) -> Vec<f32> {
        if hills.is_empty() {
            return vec![];
        }

        // 1. Find the maximum end time
        let max_end_time = hills.par_iter()
            .map(|h| h.times.last().copied().unwrap_or(0.0))
            .reduce(|| 0.0, f64::max);

        let total_samples = (max_end_time * self.sample_rate as f64).ceil() as usize;
        if total_samples == 0 {
            return vec![];
        }

        println!("      Audio duration: {:.2} minutes", max_end_time / 60.0);
        println!("      Total samples to render: {}", total_samples);

        // 2. Spatial-Temporal Partitioning (Bucket Sort)
        // We divide the audio timeline into "buckets" of 1 second each.
        // A hill is placed into every bucket that it overlaps with.
        let bucket_duration = 1.0; // 1 second
        let num_buckets = (max_end_time / bucket_duration).ceil() as usize + 1;
        
        println!("      Partitioning {} hills into {} temporal buckets...", hills.len(), num_buckets);
        
        // Use a vector of vectors for buckets.
        let mut buckets: Vec<Vec<&Hill>> = vec![Vec::new(); num_buckets];
        
        for hill in &hills {
            let start_t = *hill.times.first().unwrap();
            let end_t = *hill.times.last().unwrap();
            
            let start_bucket = (start_t / bucket_duration).floor() as usize;
            let end_bucket = (end_t / bucket_duration).floor() as usize;
            
            for b in start_bucket..=end_bucket {
                if b < num_buckets {
                    buckets[b].push(hill);
                }
            }
        }

        // 3. Initialize the global audio buffer
        let mut final_buffer = vec![0.0f32; total_samples];
        let chunk_size = (bucket_duration * self.sample_rate as f64) as usize;

        println!("      Rendering buckets in parallel...");

        // 4. Render Chunks in Parallel
        // Now, instead of iterating over ALL hills for each chunk, 
        // a thread only iterates over the hills assigned to its specific time bucket.
        final_buffer.par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(bucket_idx, chunk)| {
                let chunk_start_idx = bucket_idx * chunk_size;
                let chunk_end_idx = chunk_start_idx + chunk.len();
                
                // Get the hills that overlap with this specific 1-second chunk
                if bucket_idx < buckets.len() {
                    let active_hills = &buckets[bucket_idx];
                    
                    for &hill in active_hills {
                        let chunk_start_time = chunk_start_idx as f64 / self.sample_rate as f64;
                        let chunk_end_time = chunk_end_idx as f64 / self.sample_rate as f64;
                        
                        // Only generate the audio for this specific chunk's time window
                        let audio = Oscillator::generate_window(hill, self.sample_rate, chunk_start_time, chunk_end_time);
                        
                        let hill_start_idx = audio.start_index;
                        let hill_len = audio.samples.len();
                        
                        // Find the overlapping index range
                        let overlap_start = chunk_start_idx.max(hill_start_idx);
                        let overlap_end = chunk_end_idx.min(hill_start_idx + hill_len);
                        
                        if overlap_start < overlap_end {
                            for global_idx in overlap_start..overlap_end {
                                let local_chunk_idx = global_idx - chunk_start_idx;
                                let local_audio_idx = global_idx - hill_start_idx;
                                
                                chunk[local_chunk_idx] += audio.samples[local_audio_idx];
                            }
                        }
                    }
                }
            });

        // 5. Final Step: Normalize
        println!("      Normalizing audio...");
        Self::normalize(&mut final_buffer);

        final_buffer
    }

    fn normalize(buffer: &mut [f32]) {
        let max_amp = buffer.par_iter()
            .map(|&s| s.abs())
            .reduce(|| 0.0f32, f32::max);

        if max_amp > 1e-6 {
            let scale = 0.9 / max_amp;
            buffer.par_iter_mut().for_each(|s| *s *= scale);
        }
    }
}
