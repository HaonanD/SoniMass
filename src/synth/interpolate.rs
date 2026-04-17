pub struct PchipInterpolator {
    times: Vec<f64>,
    intensities: Vec<f32>,
    slopes: Vec<f32>,
}

impl PchipInterpolator {
    /// Creates a new PCHIP (Piecewise Cubic Hermite Interpolating Polynomial) interpolator.
    /// Ensures that the interpolated curve preserves monotonicity and avoids overshoot.
    pub fn new(times: &[f64], intensities: &[f32]) -> Self {
        assert_eq!(times.len(), intensities.len(), "Times and intensities must have the same length");
        let n = times.len();

        if n == 0 {
            return Self {
                times: vec![],
                intensities: vec![],
                slopes: vec![],
            };
        }

        if n == 1 {
            return Self {
                times: times.to_vec(),
                intensities: intensities.to_vec(),
                slopes: vec![0.0],
            };
        }

        // Calculate differences
        let mut h = Vec::with_capacity(n - 1);
        let mut delta = Vec::with_capacity(n - 1);
        for i in 0..(n - 1) {
            let dx = times[i + 1] - times[i];
            let dy = intensities[i + 1] - intensities[i];
            h.push(dx as f32);
            delta.push(if dx > 0.0 { dy / (dx as f32) } else { 0.0 });
        }

        let mut slopes = vec![0.0; n];

        if n == 2 {
            slopes[0] = delta[0];
            slopes[1] = delta[0];
        } else {
            // Interior slopes using harmonic mean
            for k in 1..(n - 1) {
                if delta[k - 1] * delta[k] <= 0.0 {
                    slopes[k] = 0.0; // Local extremum or flat, slope must be 0
                } else {
                    let w1 = 2.0 * h[k] + h[k - 1];
                    let w2 = h[k] + 2.0 * h[k - 1];
                    slopes[k] = (w1 + w2) / (w1 / delta[k - 1] + w2 / delta[k]);
                }
            }

            // Endpoint slopes (one-sided approximation)
            slopes[0] = Self::endpoint_slope(h[0], h[1], delta[0], delta[1]);
            slopes[n - 1] = Self::endpoint_slope(h[n - 2], h[n - 3], delta[n - 2], delta[n - 3]);
        }

        Self {
            times: times.to_vec(),
            intensities: intensities.to_vec(),
            slopes,
        }
    }

    fn endpoint_slope(h1: f32, h2: f32, del1: f32, del2: f32) -> f32 {
        let d = ((2.0 * h1 + h2) * del1 - h1 * del2) / (h1 + h2);
        if d * del1 <= 0.0 {
            0.0
        } else if del1 * del2 <= 0.0 && d.abs() > (3.0 * del1).abs() {
            3.0 * del1
        } else {
            d
        }
    }

    /// Evaluates the interpolated value at time `t`.
    pub fn get_value(&self, t: f64) -> f32 {
        let n = self.times.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 || t <= self.times[0] {
            return self.intensities[0];
        }
        if t >= self.times[n - 1] {
            return self.intensities[n - 1];
        }

        // Binary search to find the correct interval [x_k, x_{k+1}]
        let k = match self.times.binary_search_by(|x| x.partial_cmp(&t).unwrap()) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };

        let x_k = self.times[k];
        let x_k1 = self.times[k + 1];
        let y_k = self.intensities[k];
        let y_k1 = self.intensities[k + 1];
        let d_k = self.slopes[k];
        let d_k1 = self.slopes[k + 1];

        let h = (x_k1 - x_k) as f32;
        let s = ((t - x_k) as f32) / h; // Normalized time [0, 1]

        // Cubic Hermite basis functions
        let h00 = 2.0 * s * s * s - 3.0 * s * s + 1.0;
        let h10 = s * s * s - 2.0 * s * s + s;
        let h01 = -2.0 * s * s * s + 3.0 * s * s;
        let h11 = s * s * s - s * s;

        // Evaluate polynomial
        let value = h00 * y_k + h10 * h * d_k + h01 * y_k1 + h11 * h * d_k1;

        // Ensure non-negativity (intensity cannot be negative)
        value.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pchip_interpolation() {
        // Simple peak: goes up and down
        let times = vec![0.0, 1.0, 2.0];
        let intensities = vec![0.0, 100.0, 0.0];
        
        let pchip = PchipInterpolator::new(&times, &intensities);

        // Check exact points
        assert_eq!(pchip.get_value(0.0), 0.0);
        assert_eq!(pchip.get_value(1.0), 100.0);
        assert_eq!(pchip.get_value(2.0), 0.0);

        // Check midpoints - must be smooth and bounded
        let mid_up = pchip.get_value(0.5);
        let mid_down = pchip.get_value(1.5);
        
        assert!(mid_up > 0.0 && mid_up < 100.0);
        assert!(mid_down > 0.0 && mid_down < 100.0);
        assert_eq!(mid_up, mid_down); // Symmetric

        // Prevent overshoot: checking a flat plateau
        let plateau_times = vec![0.0, 1.0, 2.0, 3.0];
        let plateau_intensities = vec![0.0, 50.0, 50.0, 0.0];
        let pchip_plateau = PchipInterpolator::new(&plateau_times, &plateau_intensities);
        
        // At t=1.5, standard cubic spline might overshoot > 50.0. PCHIP must NOT.
        let val_plateau = pchip_plateau.get_value(1.5);
        assert!(val_plateau <= 50.0001, "PCHIP should not overshoot the plateau. Got: {}", val_plateau);
    }
}
