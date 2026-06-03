use crate::core::config::{ColorAnchor, HeatmapConfig, IntensityConfig};
use crate::core::structs::Hill;
use png::ColorType;
use std::fs::File;
use std::io::BufWriter;

pub const HEATMAP_WIDTH: u32 = 1600;
pub const HEATMAP_HEIGHT: u32 = 800;

pub fn write_heatmap_png(
    hills: &[Hill],
    audio_duration_s: f64,
    path: &str,
    intensity_cfg: &IntensityConfig,
    heatmap_cfg: &HeatmapConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = HEATMAP_WIDTH as usize;
    let h = HEATMAP_HEIGHT as usize;

    let grid = rasterize(hills, audio_duration_s, w, h, intensity_cfg.log_offset);
    let rgb = colorize(&grid, w, h, &heatmap_cfg.anchors);

    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), w as u32, h as u32);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgb)?;

    Ok(())
}

fn rasterize(hills: &[Hill], audio_duration_s: f64, w: usize, h: usize, log_offset: f32) -> Vec<f32> {
    let mut grid = vec![0.0f32; w * h];

    if audio_duration_s <= 0.0 || hills.is_empty() {
        return grid;
    }

    for hill in hills {
        let y = mz_to_y(hill.average_mz, h);
        let n = hill.times.len();
        if n == 0 {
            continue;
        }

        if n == 1 {
            let x = time_to_x(hill.times[0], audio_duration_s, w);
            let v = (log_offset + hill.intensity_values[0]).ln();
            let idx = y * w + x;
            if v > grid[idx] {
                grid[idx] = v;
            }
            continue;
        }

        for i in 0..n - 1 {
            let t0 = hill.times[i];
            let t1 = hill.times[i + 1];
            let v0 = (log_offset + hill.intensity_values[i]).ln();
            let v1 = (log_offset + hill.intensity_values[i + 1]).ln();
            let x0 = time_to_x(t0, audio_duration_s, w);
            let x1 = time_to_x(t1, audio_duration_s, w);

            if x0 == x1 {
                let v = v0.max(v1);
                let idx = y * w + x0;
                if v > grid[idx] {
                    grid[idx] = v;
                }
            } else {
                let span = (x1 - x0) as f32;
                for x in x0..=x1 {
                    let u = (x - x0) as f32 / span;
                    let v = v0 + u * (v1 - v0);
                    let idx = y * w + x;
                    if v > grid[idx] {
                        grid[idx] = v;
                    }
                }
            }
        }
    }

    grid
}

fn colorize(grid: &[f32], w: usize, h: usize, anchors: &[ColorAnchor]) -> Vec<u8> {
    let grid_max = grid.iter().cloned().fold(0.0f32, f32::max);
    let mut rgb = vec![0u8; w * h * 3];

    let bg = colormap(0.0, anchors);
    for i in 0..w * h {
        rgb[i * 3]     = bg[0];
        rgb[i * 3 + 1] = bg[1];
        rgb[i * 3 + 2] = bg[2];
    }

    if grid_max < 1e-12 {
        return rgb;
    }

    for i in 0..w * h {
        if grid[i] > 0.0 {
            let t = grid[i] / grid_max;
            let [r, g, b] = colormap(t, anchors);
            rgb[i * 3]     = r;
            rgb[i * 3 + 1] = g;
            rgb[i * 3 + 2] = b;
        }
    }

    rgb
}

/// Maps m/z to PNG pixel Y coordinate.
/// mz=300 → bottom row (y = h-1), mz=1000 → top row (y = 0).
/// Linear in m/z (equivalent to linear in log-Hz per the synthesis mapping).
fn mz_to_y(mz: f64, h: usize) -> usize {
    let t = ((mz - 300.0) / 700.0).clamp(0.0, 1.0);
    let y = ((1.0 - t) * (h - 1) as f64).round() as usize;
    y.min(h - 1)
}

fn time_to_x(t: f64, duration: f64, w: usize) -> usize {
    let frac = (t / duration).clamp(0.0, 1.0);
    let x = (frac * (w - 1) as f64).round() as usize;
    x.min(w - 1)
}

/// Piecewise linear colormap over N anchors.
/// t=0 → darkest, t=1 → brightest. Anchors must be sorted by t.
fn colormap(t: f32, anchors: &[ColorAnchor]) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let n = anchors.len();

    let mut seg = n - 2;
    for i in 0..n - 1 {
        if t <= anchors[i + 1].t {
            seg = i;
            break;
        }
    }

    let a0 = &anchors[seg];
    let a1 = &anchors[seg + 1];
    let u = if (a1.t - a0.t).abs() > 1e-9 {
        (t - a0.t) / (a1.t - a0.t)
    } else {
        0.0
    };

    [
        (a0.rgb[0] as f32 + u * (a1.rgb[0] as f32 - a0.rgb[0] as f32)).round() as u8,
        (a0.rgb[1] as f32 + u * (a1.rgb[1] as f32 - a0.rgb[1] as f32)).round() as u8,
        (a0.rgb[2] as f32 + u * (a1.rgb[2] as f32 - a0.rgb[2] as f32)).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{HeatmapConfig, IntensityConfig};

    #[test]
    fn test_mz_to_y_boundaries() {
        let h = HEATMAP_HEIGHT as usize;
        assert_eq!(mz_to_y(300.0, h), h - 1, "mz=300 should be bottom row");
        assert_eq!(mz_to_y(1000.0, h), 0, "mz=1000 should be top row");
        let mid = mz_to_y(650.0, h);
        assert!(mid > 0 && mid < h - 1);
    }

    #[test]
    fn test_mz_to_y_clamp() {
        let h = HEATMAP_HEIGHT as usize;
        assert_eq!(mz_to_y(100.0, h), h - 1);
        assert_eq!(mz_to_y(1500.0, h), 0);
    }

    #[test]
    fn test_time_to_x_boundaries() {
        let w = HEATMAP_WIDTH as usize;
        assert_eq!(time_to_x(0.0, 60.0, w), 0);
        assert_eq!(time_to_x(60.0, 60.0, w), w - 1);
        assert_eq!(time_to_x(120.0, 60.0, w), w - 1); // clamped
    }

    #[test]
    fn test_colormap_boundaries() {
        let anchors = HeatmapConfig::default().anchors;
        let dark = colormap(0.0, &anchors);
        let bright = colormap(1.0, &anchors);
        let dark_lum: u32 = dark.iter().map(|&v| v as u32).sum();
        let bright_lum: u32 = bright.iter().map(|&v| v as u32).sum();
        assert!(bright_lum > dark_lum, "t=1 should be brighter than t=0");
    }

    #[test]
    fn test_write_heatmap_png_empty() {
        let tmp = std::env::temp_dir().join("cicada_test_empty_heatmap.png");
        let result = write_heatmap_png(
            &[], 60.0, tmp.to_str().unwrap(),
            &IntensityConfig::default(), &HeatmapConfig::default(),
        );
        assert!(result.is_ok());
    }
}
