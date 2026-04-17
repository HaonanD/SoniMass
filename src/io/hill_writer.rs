use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use crate::core::structs::Hill;

/// Write a slice of Hills to a CSV file.
/// Format: id,average_mz,time,intensity (one row per data point)
pub fn write_hills_csv(hills: &[Hill], path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(Path::new(path))?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "id,average_mz,time,intensity")?;

    for hill in hills {
        let id = hill.id;
        let mz = hill.average_mz;
        for (t, intensity) in hill.times.iter().zip(hill.intensity_values.iter()) {
            writeln!(writer, "{},{:.6},{:.6},{:.4}", id, mz, t, intensity)?;
        }
    }

    writer.flush()?;
    Ok(())
}
