use clap::{Parser, ValueEnum};
use cicada::algo::hill_builder::HillBuilder;
use cicada::io::audio_writer::AudioWriter;
use cicada::io::mzml_reader::MzmlReader;
use cicada::synth::synthesizer::Synthesizer;
use cicada::core::structs::Hill;
use std::path::Path;

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    println!("Cicada (SoniMass) - Processing Started");
    println!("  Input: {}", cli.input);
    println!("  Mode: {:?}", cli.mode);
    println!("  PPM Tolerance: {}", cli.ppm);
    println!("  Min Hill Length: {}", cli.min_len);
    println!("  MS Level Filter: {:?}", cli.mslevel);
    println!("  Time Speedup: {}x", cli.speed);

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
        
        println!("[3/5] Synthesizing MS1 Audio...");
        let synth = Synthesizer::new(sample_rate);
        let audio = synth.render(hills);
        
        let out = format!("{}_ms1.wav", cli.output);
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
        
        println!("[5/5] Synthesizing MS2 Audio...");
        let synth = Synthesizer::new(sample_rate);
        let audio = synth.render(hills);
        
        let out = format!("{}_ms2.wav", cli.output);
        println!("      Writing to {}...", out);
        let writer = AudioWriter::new(&out, sample_rate);
        writer.write_buffer(&audio)?;
    } else {
        println!("[4/5] Skipping MS2 Track (No data or DDA mode)");
        println!("[5/5] Skipping MS2 Audio Synthesis");
    }

    println!("Done! 🎵");
    Ok(())
}
