use crate::core::structs::{Peak, Spectrum};
use mzdata::io::mzml::MzMLReader;
use mzdata::prelude::*;
use std::fs::File;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MzmlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("MzML read error: {0}")]
    MzDataError(String),
}

pub struct MzmlReader {
    reader: MzMLReader<File>,
}

impl MzmlReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, MzmlError> {
        let file = File::open(path)?;
        let reader = MzMLReader::new(file);
        Ok(Self { reader })
    }

    pub fn iter(self) -> MzmlIterator {
        MzmlIterator {
            reader: self.reader,
        }
    }
}

pub struct MzmlIterator {
    reader: MzMLReader<File>,
}

impl Iterator for MzmlIterator {
    type Item = Result<Spectrum, MzmlError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.next() {
            Some(mz_spectrum) => {
                let ms_level = mz_spectrum.ms_level();
                let time = mz_spectrum.start_time() * 60.0; // Convert minutes to seconds
                let index = mz_spectrum.index();

                // mzdata handles decoding (zlib, numpress, float/int conversion) automatically
                let peaks_raw = mz_spectrum.peaks();
                let mut peaks = Vec::with_capacity(peaks_raw.len());

                for p in peaks_raw.iter() {
                    peaks.push(Peak {
                        mz: p.mz,
                        intensity: p.intensity,
                    });
                }

                Some(Ok(Spectrum {
                    index,
                    time,
                    ms_level,
                    peaks,
                }))
            }
            None => None,
        }
    }
}
