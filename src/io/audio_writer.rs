use hound;
use std::path::Path;

pub struct AudioWriter {
    path: String,
    sample_rate: u32,
}

impl AudioWriter {
    pub fn new(path: &str, sample_rate: u32) -> Self {
        Self {
            path: path.to_string(),
            sample_rate,
        }
    }

    /// Writes a slice of f32 samples (normalized to [-1.0, 1.0]) to a WAV file.
    pub fn write_buffer(&self, buffer: &[f32]) -> Result<(), hound::Error> {
        let spec = hound::WavSpec {
            channels: 1, // Mono
            sample_rate: self.sample_rate,
            bits_per_sample: 32, // High fidelity floating point
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(Path::new(&self.path), spec)?;

        for &sample in buffer {
            // Write directly as f32 since we configured SampleFormat::Float
            writer.write_sample(sample)?;
        }

        writer.finalize()?;
        Ok(())
    }
}
