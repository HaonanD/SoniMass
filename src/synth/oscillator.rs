use crate::core::structs::Hill;
use crate::synth::interpolate::PchipInterpolator;
use std::f32::consts::PI;

pub struct GeneratedAudio {
    pub start_index: usize,
    pub samples: Vec<f32>,
}

pub struct Oscillator;

impl Oscillator {
    /// Generates audio samples strictly within the requested time window [window_start_time, window_end_time].
    /// Returns a struct containing the start index in the global buffer and the generated samples.
    pub fn generate_window(hill: &Hill, sample_rate: u32, window_start_time: f64, window_end_time: f64) -> GeneratedAudio {
        let f = Self::mz_to_freq(hill.average_mz);
        let pchip = PchipInterpolator::new(&hill.times, &hill.intensity_values);
        
        let hill_start_time = *hill.times.first().unwrap_or(&0.0);
        let hill_end_time = *hill.times.last().unwrap_or(&0.0);
        
        // Find the overlapping time
        let actual_start_time = window_start_time.max(hill_start_time);
        let actual_end_time = window_end_time.min(hill_end_time);
        
        if actual_start_time >= actual_end_time {
             return GeneratedAudio { start_index: 0, samples: vec![] };
        }

        let start_idx = (actual_start_time * sample_rate as f64).floor() as usize;
        let end_idx = (actual_end_time * sample_rate as f64).ceil() as usize;
        
        let num_samples = if end_idx > start_idx { end_idx - start_idx } else { 0 };
        let mut samples = Vec::with_capacity(num_samples);
        
        for i in 0..num_samples {
            let current_idx = start_idx + i;
            let t = current_idx as f64 / sample_rate as f64;
            
            // 1. Get interpolated amplitude
            let amp = pchip.get_value(t);
            
            // 2. Sine wave calculation: A * sin(2 * pi * f * t)
            // Using linear amplitude mapping to preserve the extreme dynamic range of MS data.
            let sample = amp * (2.0 * PI * f * t as f32).sin();
            samples.push(sample);
        }
        
        GeneratedAudio {
            start_index: start_idx,
            samples,
        }
    }

    /// Forward Linear Mapping: Maps m/z to Frequency (Hz)
    /// Range: [300, 1000] m/z -> [30, 4200] Hz
    fn mz_to_freq(mz: f64) -> f32 {
        let min_mz = 300.0;
        let max_mz = 1000.0;
        let min_freq = 30.0;
        let max_freq = 4200.0;
        
        // Clamp mz to prevent extreme frequencies
        let clamped_mz = mz.max(min_mz).min(max_mz);
        
        let freq = min_freq + (max_freq - min_freq) * (clamped_mz - min_mz) / (max_mz - min_mz);
        freq as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::structs::Peak;

    #[test]
    fn test_mz_mapping() {
        // Test midpoint: (1000+300)/2 = 650 m/z
        // (4200+30)/2 = 2115 Hz
        let freq = Oscillator::mz_to_freq(650.0);
        assert!((freq - 2115.0).abs() < 1.0);
        
        // Test boundaries
        assert_eq!(Oscillator::mz_to_freq(300.0), 30.0);
        assert_eq!(Oscillator::mz_to_freq(1000.0), 4200.0);
    }

    #[test]
    fn test_oscillator_generation() {
        let hill = Hill::new(
            0, 
            Peak { mz: 650.0, intensity: 1.0 }, 
            0, 
            0.0
        );
        // Add one more point at 1 second
        let mut hill = hill;
        hill.push(Peak { mz: 650.0, intensity: 1.0 }, 1, 1.0);
        
        let audio = Oscillator::generate_window(&hill, 44100, 0.0, 1.0);
        
        assert_eq!(audio.start_index, 0);
        // 1 second at 44100Hz = 44101 samples (due to ceil/floor logic)
        assert!(audio.samples.len() >= 44100);
        
        // Check if we have non-zero samples
        let has_signal = audio.samples.iter().any(|&s| s.abs() > 0.0);
        assert!(has_signal);
    }
}
