// src/dataset.rs
use crate::types::{TimsFfiSpectrum, TimsFfiSwathWindow, TimsFfiStatus};
use std::ffi::CStr;
use std::ptr;

// Make the dependency on the external `timsrust` crate optional so we can
// build and test the FFI surface even when timsrust is not available in the
// current environment. When the `with_timsrust` feature is enabled we use
// the real reader; otherwise provide a small stub with the same surface.

#[cfg(feature = "with_timsrust")]
use timsrust::readers::SpectrumReader;
#[cfg(feature = "with_timsrust")]
use timsrust::readers::MetadataReader;

#[cfg(not(feature = "with_timsrust"))]
struct SpectrumReader {
    // minimal stub keeps a length (0) to allow basic API tests
    n: usize,
}

#[cfg(not(feature = "with_timsrust"))]
impl SpectrumReader {
    fn new(_path: &CStr) -> Result<Self, ()> {
        Ok(SpectrumReader { n: 0 })
    }

    fn len(&self) -> usize {
        self.n
    }
}

pub struct TimsDataset {
    reader: SpectrumReader,
    // Reusable buffers for mz/intensity to keep ownership in Rust
    mz_buf: Vec<f32>,
    int_buf: Vec<f32>,
    // Later: cached DIA windows, MS1/MS2 counts, etc.
}

impl TimsDataset {
    pub fn open(path: &CStr) -> Result<Self, TimsFfiStatus> {
        let path_str = path.to_str().map_err(|_| TimsFfiStatus::InvalidUtf8)?;
        // When using the real timsrust reader, SpectrumReader::new accepts a
        // &str and returns a Result<SpectrumReader, _>. Our stub above uses
        // a CStr-based new signature; handle both surfaces.
        #[cfg(feature = "with_timsrust")]
        let reader = SpectrumReader::new(path_str)
            .map_err(|_| TimsFfiStatus::OpenFailed)?;

        #[cfg(not(feature = "with_timsrust"))]
        let reader = SpectrumReader::new(path)
            .map_err(|_| TimsFfiStatus::OpenFailed)?;
        Ok(TimsDataset {
            reader,
            mz_buf: Vec::new(),
            int_buf: Vec::new(),
        })
    }

    pub fn len(&self) -> u32 {
        self.reader.len() as u32
    }

    pub fn get_spectrum(&mut self, index: u32, out: &mut TimsFfiSpectrum)
        -> Result<(), TimsFfiStatus>
    {
        // Basic safety: check bounds using the reader's length.
        let len = self.reader.len() as u32;
        if index >= len {
            return Err(TimsFfiStatus::IndexOutOfBounds);
        }

        // Real implementation when timsrust feature is enabled.
        #[cfg(feature = "with_timsrust")]
        {
            // Fetch spectrum from timsrust
            let spec = self.reader.get(index as usize)
                .map_err(|_| TimsFfiStatus::IndexOutOfBounds)?;

            // Convert mz/int to f32 buffers owned by this dataset handle.
            let n = spec.len();
            self.mz_buf.clear();
            self.mz_buf.reserve(n);
            for &v in spec.mz_values.iter() {
                self.mz_buf.push(v as f32);
            }
            self.int_buf.clear();
            self.int_buf.reserve(n);
            for &v in spec.intensities.iter() {
                self.int_buf.push(v as f32);
            }

            // Fill output struct
            out.num_peaks = n as u32;
            out.mz = if n == 0 { ptr::null() } else { self.mz_buf.as_ptr() };
            out.intensity = if n == 0 { ptr::null() } else { self.int_buf.as_ptr() };

            // Metadata: try to extract precursor RT/IM if available
            if let Some(prec) = spec.precursor {
                out.rt_seconds = prec.rt;
                out.precursor_mz = prec.mz;
                out.im = prec.im;
                out.ms_level = 2;
            } else {
                out.rt_seconds = 0.0;
                out.precursor_mz = 0.0;
                out.im = 0.0;
                out.ms_level = 1;
            }

            return Ok(());
        }

        // Fallback minimal implementation when timsrust is not enabled.
        #[cfg(not(feature = "with_timsrust"))]
        {
            out.rt_seconds = 0.0;
            out.precursor_mz = 0.0;
            out.ms_level = 0;
            out.num_peaks = 0;
            out.mz = ptr::null();
            out.intensity = ptr::null();
            out.im = 0.0;
            Ok(())
        }
    }

    pub fn get_swath_windows(&self) -> Result<Vec<TimsFfiSwathWindow>, TimsFfiStatus> {
        // Use timsrust's QuadrupoleSettingsReader / MetadataReader here
        // ...
        Ok(vec![])
    }
}

