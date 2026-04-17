use crate::core::structs::{Hill, Spectrum};

pub struct HillBuilder {
    /// Max allowed missing scans before a hill is considered 'finished'
    pub max_gap: usize,
    /// Tolerance for m/z matching (in ppm)
    pub ppm_tolerance: f64,
    /// Minimum number of points required to keep a hill
    pub min_length: usize,
}

impl HillBuilder {
    pub fn new() -> Self {
        Self {
            max_gap: 1, // Default from Dinosaur (allows skipping 1 scan)
            ppm_tolerance: 10.0,
            min_length: 5, // Default threshold for sonification
        }
    }

    pub fn with_ppm_tolerance(mut self, ppm: f64) -> Self {
        self.ppm_tolerance = ppm;
        self
    }

    pub fn with_max_gap(mut self, gap: usize) -> Self {
        self.max_gap = gap;
        self
    }

    pub fn with_min_length(mut self, min_len: usize) -> Self {
        self.min_length = min_len;
        self
    }

    /// The core function: Process a stream of spectra and produce Hills.
    /// This implements the "Two-Pointer Greedy Matching" logic.
    pub fn process_stream<I>(&self, spectra: I) -> Vec<Hill>
    where
        I: Iterator<Item = Spectrum>,
    {
        let mut active_hills: Vec<Hill> = Vec::new();
        let mut completed_hills: Vec<Hill> = Vec::new();
        let mut next_hill_id = 0;

        for spectrum in spectra {
            let current_scan_idx = spectrum.index;
            let current_time = spectrum.time;
            
            // 1. Sort active hills by mz_guess to enable greedy linear matching
            // Using sort_by with partial_cmp because f64 is not Ord
            active_hills.sort_by(|a, b| a.mz_guess.partial_cmp(&b.mz_guess).unwrap());

            // 2. Prepare for matching
            let mut next_active_hills: Vec<Hill> = Vec::with_capacity(active_hills.len());
            let mut peak_cursor = 0;
            // NOTE: Spectrum peaks MUST be sorted by m/z for this greedy matching to work.
            let peaks = &spectrum.peaks;
            
            // 3. The Two-Pointer Dance
            let mut hill_cursor = 0;
            
            while hill_cursor < active_hills.len() && peak_cursor < peaks.len() {
                let hill = &mut active_hills[hill_cursor];
                let peak = &peaks[peak_cursor];

                // Calculate signed ppm difference (relative to theoretical mz)
                let ppm = (hill.mz_guess - peak.mz) / hill.mz_guess * 1_000_000.0;

                if ppm.abs() <= self.ppm_tolerance {
                    // MATCH!
                    // Extend the hill
                    hill.push(*peak, current_scan_idx, current_time);
                    
                    // Greedy: Move both cursors
                    hill_cursor += 1;
                    peak_cursor += 1;
                } else if hill.mz_guess < peak.mz {
                    // Hill is "behind" the current peak m/z. 
                    // This hill missed a match in this scan (Gap).
                    hill_cursor += 1;
                } else {
                    // Peak is "behind" the current hill m/z.
                    // This is a NEW feature starting.
                    let new_hill = Hill::new(next_hill_id, *peak, current_scan_idx, current_time);
                    next_active_hills.push(new_hill);
                    next_hill_id += 1;
                    
                    peak_cursor += 1;
                }
            }

            // 4. Handle remaining Peaks (New Hills)
            while peak_cursor < peaks.len() {
                let peak = &peaks[peak_cursor];
                let new_hill = Hill::new(next_hill_id, *peak, current_scan_idx, current_time);
                next_active_hills.push(new_hill);
                next_hill_id += 1;
                peak_cursor += 1;
            }

            // 5. Handle active hills (Check Gaps & Move to next frame)
            // We iterate through the OLD active list.
            // If matched: It was already pushed to matched_hills? No, we need to move it.
            // Wait, the ownership logic above is tricky with the mutable borrow in the loop.
            // Rust ownership makes the "Modify in place then move" pattern hard.
            // Let's adopt a different strategy: 
            // We matched in-place in `active_hills` (using indices). Now we decide who stays.
            
            // Re-iterate all active hills to decide their fate
            for hill in active_hills.drain(..) {
                // Was it matched in this scan?
                if hill.last_scan_index == current_scan_idx {
                    // Yes, keep it alive
                    next_active_hills.push(hill);
                } else {
                    // No, it's a Gap. Check if we should kill it.
                    let gap = current_scan_idx - hill.last_scan_index;
                    if gap > self.max_gap {
                        // Retired
                        if hill.scan_indices.len() >= self.min_length {
                            completed_hills.push(hill);
                        }
                    } else {
                        // Gap Skipping: Keep it alive, maybe it appears in next scan
                        next_active_hills.push(hill);
                    }
                }
            }

            active_hills = next_active_hills;
        }

        // Finish remaining active hills after file ends
        for hill in active_hills {
            if hill.scan_indices.len() >= self.min_length {
                completed_hills.push(hill);
            }
        }
        
        completed_hills
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::structs::{Peak, Spectrum};

    #[test]
    fn test_visualize_hill_building() {
        // 使用默认配置 (10 ppm 容差), 但为了测试短信号将 min_length 设为 2
        let builder = HillBuilder::new().with_min_length(2);

        // 模拟两个化合物（使用符合高分辨质谱的真实微小抖动）：
        // 化合物 A: m/z 100.0 左右。10 ppm 允许的偏差是 0.001 Da。
        // 化合物 B: m/z 200.0 左右。10 ppm 允许的偏差是 0.002 Da。
        
        let spectra = vec![
            Spectrum { 
                index: 1, time: 1.0, ms_level: 1, 
                peaks: vec![Peak { mz: 100.0, intensity: 10.0 }] 
            },
            Spectrum { 
                index: 2, time: 2.0, ms_level: 1, 
                peaks: vec![
                    Peak { mz: 100.0005, intensity: 50.0 }, // A 发生真实微小抖动 (+0.0005)
                    Peak { mz: 200.0, intensity: 100.0 }    // B 出现
                ] 
            },
            Spectrum { 
                index: 3, time: 3.0, ms_level: 1, 
                peaks: vec![
                    Peak { mz: 99.9995, intensity: 100.0 }, // A 发生真实微小抖动 (-0.0005)
                    Peak { mz: 200.001, intensity: 80.0 }   // B 发生真实微小抖动 (+0.001)
                ] 
            },
            Spectrum { 
                index: 4, time: 4.0, ms_level: 1, 
                peaks: vec![Peak { mz: 100.0, intensity: 20.0 }] // 只有 A
            },
        ];

        let hills = builder.process_stream(spectra.into_iter());

        println!("\n--- Hill Building 验证结果 (Realistic 10 ppm Tolerance) ---");
        println!("总共生成了 {} 条轨迹", hills.len());
        for hill in &hills {
            println!("轨迹 ID: {}", hill.id);
            println!("  加权平均 m/z: {:.4}", hill.average_mz);
            println!("  覆盖扫描帧: {:?}", hill.scan_indices);
            println!("  强度分布: {:?}", hill.intensity_values);
            println!("----------------------------");
        }

        // 预期：算法能成功在 10 ppm 内抵抗这些微小抖动，将它们缝合为 2 条完美的轨迹
        assert_eq!(hills.len(), 2);
    }
}
