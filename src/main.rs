use clap::{Parser, ValueEnum};
use cicada::algo::hill_builder::HillBuilder;
use cicada::core::config::Config;
use cicada::io::audio_writer::AudioWriter;
use cicada::io::heatmap_writer::{write_heatmap_png, HEATMAP_HEIGHT, HEATMAP_WIDTH};
use cicada::io::hill_writer::write_hills_csv;
use cicada::io::html_writer::write_heatmap_html;
use cicada::io::mzml_reader::MzmlReader;
use cicada::synth::synthesizer::Synthesizer;
use cicada::core::structs::Hill;
use std::path::Path;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_SHA: &str = env!("VERGEN_GIT_SHA");
const GIT_DIRTY: &str = env!("VERGEN_GIT_DIRTY");

#[derive(Clone, Debug, ValueEnum, PartialEq)]
enum Mode {
    Dia,
    Dda,
}

#[derive(Clone, Debug, PartialEq)]
enum MsLevelFilter {
    Ms1,
    Ms2,
    All,
}

impl std::str::FromStr for MsLevelFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "1" | "ms1" => Ok(MsLevelFilter::Ms1),
            "2" | "ms2" => Ok(MsLevelFilter::Ms2),
            "all" => Ok(MsLevelFilter::All),
            _ => Err(format!("Invalid MS level: {}. Use 1, 2, or all", s)),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "SoniMass: Mass Spectrometry Sonification", long_about = None)]
struct Cli {
    /// Input mzML file path
    input: String,

    /// Output file prefix
    #[arg(short, long, default_value = "output")]
    output: String,

    /// PPM tolerance for Hill Building
    #[arg(long, default_value_t = 10.0)]
    ppm: f64,

    /// Acquisition mode: dia or dda (affects whether MS2 is processed)
    #[arg(long, value_enum, default_value_t = Mode::Dia)]
    mode: Mode,

    /// MS level to process: 1, 2, or all
    #[arg(long, default_value = "all")]
    mslevel: MsLevelFilter,
    
    /// Minimum number of data points for a valid hill
    #[arg(long, default_value_t = 5)]
    min_len: usize,
    
    /// Time scaling factor (e.g., 60.0 to compress 60 mins into 1 min)
    #[arg(long, default_value_t = 1.0)]
    speed: f64,

    /// Skip exporting hill data to CSV (default: hills are exported)
    #[arg(long, default_value_t = false)]
    no_export_hills: bool,

    /// Skip exporting heatmap PNG and HTML viewer (default: viz is exported)
    #[arg(long, default_value_t = false)]
    no_export_viz: bool,

    /// Signal mapping scheme: 1 = log m/z→freq + ln(1+I) amplitude (default)
    #[arg(long, default_value_t = 1)]
    mapping_method: u32,

    /// Path to TOML config file for mapping parameters (optional; uses built-in defaults if omitted)
    #[arg(long)]
    config: Option<String>,

    /// Start of time selection range, in minutes (default: beginning of data)
    #[arg(long)]
    start: Option<f64>,

    /// Width of time selection range, in minutes (default: to end of data)
    #[arg(long)]
    width: Option<f64>,

}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.mapping_method == 0 || cli.mapping_method > 1 {
        eprintln!("Error: --mapping_method must be 1 (got {}). Only method 1 is currently supported.", cli.mapping_method);
        std::process::exit(1);
    }

    let cfg = match Config::load(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config error: {}", e);
            std::process::exit(1);
        }
    };

    println!("Cicada (SoniMass) - Processing Started");
    println!("  Input: {}", cli.input);
    match &cli.config {
        Some(p) => println!("  Config: {}", p),
        None    => println!("  Config: built-in defaults"),
    }
    println!("  Mode: {:?}", cli.mode);
    println!("  Mapping Method: {}", cli.mapping_method);
    println!("  PPM Tolerance: {}", cli.ppm);
    println!("  Min Hill Length: {}", cli.min_len);
    println!("  MS Level Filter: {:?}", cli.mslevel);
    println!("  Time Speedup: {}x", cli.speed);

    // Validate and compute time-range filter (convert minutes → seconds)
    if let Some(s) = cli.start {
        if s < 0.0 {
            eprintln!("Error: --start must be >= 0 (got {})", s);
            std::process::exit(1);
        }
    }
    if let Some(w) = cli.width {
        if w <= 0.0 {
            eprintln!("Error: --width must be > 0 (got {})", w);
            std::process::exit(1);
        }
    }
    let start_s = cli.start.unwrap_or(0.0) * 60.0;
    let end_s: Option<f64> = cli.width.map(|w| start_s + w * 60.0);

    if cli.start.is_some() || cli.width.is_some() {
        let end_label = end_s
            .map(|e| format!("{:.2} min", e / 60.0))
            .unwrap_or_else(|| "end".to_string());
        println!("  Time Range: [{:.2} min, {})", start_s / 60.0, end_label);
    }

    let path = Path::new(&cli.input);
    let sample_rate = 44100;

    if !path.exists() {
        eprintln!("Error: Input file {:?} not found.", path);
        std::process::exit(1);
    }

    // 1. Parse and Categorize Spectra
    println!("[1/5] Parsing mzML...");
    let reader = MzmlReader::new(path)?;
    let mut ms1_spectra = Vec::new();
    let mut ms2_spectra = Vec::new();

    for result in reader.iter() {
        let spec = result?;

        // Time-range filter
        if spec.time < start_s {
            continue;
        }
        if let Some(e) = end_s {
            if spec.time >= e {
                continue;
            }
        }

        // Filter based on --mslevel
        match cli.mslevel {
            MsLevelFilter::Ms1 if spec.ms_level != 1 => continue,
            MsLevelFilter::Ms2 if spec.ms_level != 2 => continue,
            _ => {} // All or matches
        }

        if spec.ms_level == 1 {
            ms1_spectra.push(spec);
        } else if spec.ms_level == 2 {
            // Only process MS2 if we are in DIA mode, or if the user explicitly requested MS2
            if cli.mode == Mode::Dia || cli.mslevel == MsLevelFilter::Ms2 {
                ms2_spectra.push(spec);
            }
        }
    }
    
    println!("      Extracted {} MS1 spectra, {} MS2 spectra", ms1_spectra.len(), ms2_spectra.len());

    if ms1_spectra.is_empty() && ms2_spectra.is_empty() {
        println!("No spectra found matching criteria. Exiting.");
        return Ok(());
    }

    // 2. Determine Global Start Time Offset
    let min_time = ms1_spectra.iter().chain(ms2_spectra.iter())
        .map(|s| s.time)
        .fold(f64::INFINITY, f64::min);
    
    println!("      Global Start Time Offset: {:.2}s (will be normalized to 0s)", min_time);

    let normalize_times = |mut hills: Vec<Hill>| {
        for h in &mut hills {
            for t in &mut h.times {
                *t = (*t - min_time) / cli.speed;
            }
        }
        hills
    };

    // 3. Process MS1
    if !ms1_spectra.is_empty() {
        println!("[2/5] Processing MS1 Track...");
        let builder = HillBuilder::new()
            .with_ppm_tolerance(cli.ppm)
            .with_min_length(cli.min_len);
        let hills = builder.process_stream(ms1_spectra.into_iter());
        println!("      Built {} MS1 hills (length >= {})", hills.len(), cli.min_len);
        
        let hills = normalize_times(hills);

        if !cli.no_export_hills {
            let hills_path = format!("{}_ms1_hills.csv", cli.output);
            println!("      Exporting MS1 hills to {}...", hills_path);
            write_hills_csv(&hills, &hills_path)?;
        }

        let out = format!("{}_ms1.wav", cli.output);

        if !cli.no_export_viz {
            let audio_duration_s = hills.iter()
                .filter_map(|h| h.times.last().copied())
                .fold(0.0_f64, f64::max);
            let png_path = format!("{}_ms1_heatmap.png", cli.output);
            let html_path = format!("{}_ms1.html", cli.output);
            println!("      Exporting MS1 heatmap to {}...", png_path);
            write_heatmap_png(&hills, audio_duration_s, &png_path,
                              &cfg.intensity, &cfg.heatmap)?;
            println!("      Exporting MS1 HTML viewer to {}...", html_path);
            let png_basename = Path::new(&png_path).file_name().unwrap().to_string_lossy();
            let wav_basename = Path::new(&out).file_name().unwrap().to_string_lossy();
            write_heatmap_html(&png_basename, &wav_basename, audio_duration_s,
                               HEATMAP_WIDTH, HEATMAP_HEIGHT, &html_path, &cfg.frequency)?;
        }

        println!("[3/5] Synthesizing MS1 Audio...");
        let synth = Synthesizer::new(sample_rate, cfg.clone());
        let audio = synth.render(hills);

        println!("      Writing to {}...", out);
        let writer = AudioWriter::new(&out, sample_rate);
        writer.write_buffer(&audio)?;
    } else {
        println!("[2/5] Skipping MS1 Track (No data)");
        println!("[3/5] Skipping MS1 Audio Synthesis");
    }

    // 4. Process MS2
    if !ms2_spectra.is_empty() {
        println!("[4/5] Processing MS2 Track...");
        let builder = HillBuilder::new()
            .with_ppm_tolerance(cli.ppm)
            .with_min_length(cli.min_len);
        let hills = builder.process_stream(ms2_spectra.into_iter());
        println!("      Built {} MS2 hills (length >= {})", hills.len(), cli.min_len);
        
        let hills = normalize_times(hills);

        if !cli.no_export_hills {
            let hills_path = format!("{}_ms2_hills.csv", cli.output);
            println!("      Exporting MS2 hills to {}...", hills_path);
            write_hills_csv(&hills, &hills_path)?;
        }

        let out = format!("{}_ms2.wav", cli.output);

        if !cli.no_export_viz {
            let audio_duration_s = hills.iter()
                .filter_map(|h| h.times.last().copied())
                .fold(0.0_f64, f64::max);
            let png_path = format!("{}_ms2_heatmap.png", cli.output);
            let html_path = format!("{}_ms2.html", cli.output);
            println!("      Exporting MS2 heatmap to {}...", png_path);
            write_heatmap_png(&hills, audio_duration_s, &png_path,
                              &cfg.intensity, &cfg.heatmap)?;
            println!("      Exporting MS2 HTML viewer to {}...", html_path);
            let png_basename = Path::new(&png_path).file_name().unwrap().to_string_lossy();
            let wav_basename = Path::new(&out).file_name().unwrap().to_string_lossy();
            write_heatmap_html(&png_basename, &wav_basename, audio_duration_s,
                               HEATMAP_WIDTH, HEATMAP_HEIGHT, &html_path, &cfg.frequency)?;
        }

        println!("[5/5] Synthesizing MS2 Audio...");
        let synth = Synthesizer::new(sample_rate, cfg.clone());
        let audio = synth.render(hills);

        println!("      Writing to {}...", out);
        let writer = AudioWriter::new(&out, sample_rate);
        writer.write_buffer(&audio)?;
    } else {
        println!("[4/5] Skipping MS2 Track (No data or DDA mode)");
        println!("[5/5] Skipping MS2 Audio Synthesis");
    }

    {
        let mslevel_str = match cli.mslevel {
            MsLevelFilter::Ms1 => "1",
            MsLevelFilter::Ms2 => "2",
            MsLevelFilter::All => "all",
        };
        let mode_str = match cli.mode {
            Mode::Dia => "dia",
            Mode::Dda => "dda",
        };
        let git_sha_display = if GIT_DIRTY == "true" {
            format!("{}-dirty", GIT_SHA)
        } else {
            GIT_SHA.to_string()
        };
        let info = serde_json::json!({
            "cicada_version": PKG_VERSION,
            "git_sha": git_sha_display,
            "input": cli.input,
            "output": cli.output,
            "mode": mode_str,
            "ppm": cli.ppm,
            "min_len": cli.min_len,
            "speed": cli.speed,
            "mslevel": mslevel_str,
            "start_min": cli.start,
            "width_min": cli.width,
            "mapping_method": cli.mapping_method,
            "no_export_hills": cli.no_export_hills,
            "no_export_viz": cli.no_export_viz,
        });
        let runinfo_path = format!("{}_runinfo.json", cli.output);
        std::fs::write(&runinfo_path, serde_json::to_string_pretty(&info)?)?;
        println!("      Run info written to {}", runinfo_path);
    }

    println!("Done! 🎵");
    Ok(())
}
