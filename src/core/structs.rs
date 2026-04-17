/// The fundamental atom of Mass Spectrometry data.
#[derive(Debug, Clone, Copy)]
pub struct Peak {
    pub mz: f64,
    pub intensity: f32,
}

/// Represents a single Mass Spectrometry scan (Spectrum).
#[derive(Debug, Clone)]
pub struct Spectrum {
    /// 0-based index of the scan in the file
    pub index: usize,
    /// Retention time in seconds (or minutes, decide later)
    pub time: f64,
    /// MS Level (1 or 2)
    pub ms_level: u8,
    /// The centroided peaks in this scan
    pub peaks: Vec<Peak>,
}

/// A continuous signal track over time (also known as a Feature trace or XIC).
/// Created by connecting peaks from consecutive spectra.
#[derive(Debug, Clone)]
pub struct Hill {
    /// Unique ID for this hill
    pub id: usize,
    /// Average m/z (weighted) - used for Frequency
    pub average_mz: f64,
    /// Rolling m/z guess from the builder (internal use mostly)
    pub mz_guess: f64,
    /// The index of the last scan where this hill was seen.
    /// Crucial for efficient Gap calculation in the hot loop.
    pub last_scan_index: usize,
    
    // Sparse data storage
    pub scan_indices: Vec<usize>,
    pub times: Vec<f64>,
    pub intensity_values: Vec<f32>,
    
    // Internal state for weighted average calculation
    total_intensity_mz_product: f64,
    total_intensity: f64,
}

impl Hill {
    pub fn new(id: usize, start_peak: Peak, start_scan_idx: usize, start_time: f64) -> Self {
        Self {
            id,
            average_mz: start_peak.mz,
            mz_guess: start_peak.mz,
            last_scan_index: start_scan_idx,
            scan_indices: vec![start_scan_idx],
            times: vec![start_time],
            intensity_values: vec![start_peak.intensity],
            
            // Initialize weighted sum
            total_intensity_mz_product: start_peak.mz * (start_peak.intensity as f64),
            total_intensity: start_peak.intensity as f64,
        }
    }

    /// Add a new point to this hill and update stats
    pub fn push(&mut self, peak: Peak, scan_idx: usize, time: f64) {
        // 1. Update storage
        self.scan_indices.push(scan_idx);
        self.times.push(time);
        self.intensity_values.push(peak.intensity);
        self.last_scan_index = scan_idx;

        // 2. Update Rolling mz_guess (Simple Exponential Smoothing logic from Dinosaur)
        // Dinosaur uses: mzGuess = (mzGuess * n + mz) / (n + 1) where n is up to a limit.
        // Simplified here to a weighted update for stability.
        let n = self.scan_indices.len() as f64;
        // Giving slightly more weight to recent history for tracking, 
        // but not too much to avoid jitter.
        self.mz_guess = (self.mz_guess * (n - 1.0) + peak.mz) / n;

        // 3. Update Global Weighted Average m/z (for final Audio Frequency)
        self.total_intensity_mz_product += peak.mz * (peak.intensity as f64);
        self.total_intensity += peak.intensity as f64;
        self.average_mz = self.total_intensity_mz_product / self.total_intensity;
    }
}
