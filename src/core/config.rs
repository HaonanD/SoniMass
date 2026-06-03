use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub frequency: FrequencyConfig,
    pub intensity: IntensityConfig,
    pub amplitude: AmplitudeConfig,
    pub heatmap:   HeatmapConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FrequencyConfig {
    pub min_mz:   f64,
    pub max_mz:   f64,
    pub min_freq: f64,
    pub max_freq: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IntensityConfig {
    /// Applied as ln(log_offset + x) to compress dynamic range.
    /// Shared by audio amplitude and heatmap color. Must be > 0.
    pub log_offset: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AmplitudeConfig {
    /// Audio buffer is peak-normalized to this target. Must be in (0, 1].
    pub normalize_target: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HeatmapConfig {
    #[serde(default = "default_anchors")]
    pub anchors: Vec<ColorAnchor>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorAnchor {
    pub t:   f32,
    pub rgb: [u8; 3],
}

// ── Default impls (reproduce current hardcoded values exactly) ───────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            frequency: FrequencyConfig::default(),
            intensity: IntensityConfig::default(),
            amplitude: AmplitudeConfig::default(),
            heatmap:   HeatmapConfig::default(),
        }
    }
}

impl Default for FrequencyConfig {
    fn default() -> Self {
        Self { min_mz: 300.0, max_mz: 1000.0, min_freq: 30.0, max_freq: 4200.0 }
    }
}

impl Default for IntensityConfig {
    fn default() -> Self {
        Self { log_offset: 1.0 }
    }
}

impl Default for AmplitudeConfig {
    fn default() -> Self {
        Self { normalize_target: 0.9 }
    }
}

impl Default for HeatmapConfig {
    fn default() -> Self {
        Self { anchors: default_anchors() }
    }
}

fn default_anchors() -> Vec<ColorAnchor> {
    vec![
        ColorAnchor { t: 0.00, rgb: [13,  8,   135] },
        ColorAnchor { t: 0.20, rgb: [128, 19,  162] },
        ColorAnchor { t: 0.40, rgb: [213, 56,  109] },
        ColorAnchor { t: 0.60, rgb: [249, 131, 50]  },
        ColorAnchor { t: 0.80, rgb: [253, 201, 39]  },
        ColorAnchor { t: 1.00, rgb: [240, 249, 33]  },
    ]
}

// ── Config loading ────────────────────────────────────────────────────────────

impl Config {
    /// Load config from a TOML file path, or return defaults if `path` is None.
    pub fn load(path: Option<&str>) -> Result<Self, String> {
        match path {
            None => Ok(Self::default()),
            Some(p) => {
                let text = std::fs::read_to_string(p)
                    .map_err(|e| format!("Cannot read config '{}': {}", p, e))?;
                let cfg: Self = toml::from_str(&text)
                    .map_err(|e| format!("Config parse error in '{}': {}", p, e))?;
                cfg.validate()?;
                Ok(cfg)
            }
        }
    }

    fn validate(&self) -> Result<(), String> {
        let f = &self.frequency;
        if f.min_mz >= f.max_mz {
            return Err(format!(
                "frequency.min_mz ({}) must be < max_mz ({})", f.min_mz, f.max_mz
            ));
        }
        if f.min_freq <= 0.0 || f.min_freq >= f.max_freq {
            return Err(format!(
                "frequency.min_freq ({}) must be > 0 and < max_freq ({})", f.min_freq, f.max_freq
            ));
        }
        let i = &self.intensity;
        if i.log_offset <= 0.0 {
            return Err(format!(
                "intensity.log_offset ({}) must be > 0", i.log_offset
            ));
        }
        let a = &self.amplitude;
        if a.normalize_target <= 0.0 || a.normalize_target > 1.0 {
            return Err(format!(
                "amplitude.normalize_target ({}) must be in (0, 1]", a.normalize_target
            ));
        }
        let anchors = &self.heatmap.anchors;
        if anchors.len() < 2 {
            return Err("heatmap.anchors must have at least 2 entries".into());
        }
        if anchors[0].t.abs() > 1e-6 {
            return Err(format!(
                "heatmap.anchors first entry t must be 0.0, got {}", anchors[0].t
            ));
        }
        if (anchors[anchors.len() - 1].t - 1.0).abs() > 1e-6 {
            return Err(format!(
                "heatmap.anchors last entry t must be 1.0, got {}", anchors[anchors.len() - 1].t
            ));
        }
        for i in 0..anchors.len() - 1 {
            if anchors[i].t >= anchors[i + 1].t {
                return Err(format!(
                    "heatmap.anchors t values must be strictly increasing (index {} = {} >= index {} = {})",
                    i, anchors[i].t, i + 1, anchors[i + 1].t
                ));
            }
        }
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_none_equals_default() {
        let cfg = Config::load(None).unwrap();
        assert_eq!(cfg.frequency.min_mz, 300.0);
        assert_eq!(cfg.frequency.max_freq, 4200.0);
        assert_eq!(cfg.intensity.log_offset, 1.0);
        assert_eq!(cfg.amplitude.normalize_target, 0.9);
        assert_eq!(cfg.heatmap.anchors.len(), 6);
    }

    #[test]
    fn test_parse_toml_overrides() {
        let toml = r#"
[frequency]
min_mz   = 200.0
max_mz   = 900.0
min_freq = 50.0
max_freq = 8000.0

[intensity]
log_offset = 0.5

[amplitude]
normalize_target = 0.8

[[heatmap.anchors]]
t   = 0.0
rgb = [0, 0, 0]

[[heatmap.anchors]]
t   = 1.0
rgb = [255, 255, 255]
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        cfg.validate().unwrap();
        assert_eq!(cfg.frequency.min_mz, 200.0);
        assert_eq!(cfg.intensity.log_offset, 0.5);
        assert_eq!(cfg.heatmap.anchors.len(), 2);
        assert_eq!(cfg.heatmap.anchors[1].rgb, [255, 255, 255]);
    }

    #[test]
    fn test_validate_rejects_bad_mz_range() {
        let mut cfg = Config::default();
        cfg.frequency.min_mz = 1000.0;
        cfg.frequency.max_mz = 300.0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_too_few_anchors() {
        let mut cfg = Config::default();
        cfg.heatmap.anchors = vec![ColorAnchor { t: 0.0, rgb: [0, 0, 0] }];
        assert!(cfg.validate().is_err());
    }
}
