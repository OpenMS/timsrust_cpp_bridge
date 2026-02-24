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
#[cfg(feature = "with_timsrust")]
use timsrust::converters::ConvertableDomain;

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
    pub(crate) reader: SpectrumReader,
    // Reusable buffers for mz/intensity to keep ownership in Rust
    mz_buf: Vec<f32>,
    int_buf: Vec<f32>,
    // Optional cached swath windows computed at open time when available
    swath_windows: Option<Vec<TimsFfiSwathWindow>>,
    // Optional last error string for this handle
    pub(crate) last_error: Option<String>,
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
        // Attempt to compute swath windows when timsrust feature is enabled.
        #[cfg(feature = "with_timsrust")]
        let swath_windows = {
            // Try to build QuadrupoleSettingsReader -> QuadrupoleSettings
            use timsrust::readers::QuadrupoleSettingsReader;
            let mut out: Vec<TimsFfiSwathWindow> = Vec::new();
            if let Ok(quads) = QuadrupoleSettingsReader::new(path_str) {
                // Also try to get metadata for IM conversion
                if let Ok(metadata) = timsrust::readers::MetadataReader::new(path_str) {
                    let im_converter = metadata.im_converter;
                    for quad in quads.iter() {
                        for i in 0..quad.len() {
                            let center = quad.isolation_mz[i];
                            let width = quad.isolation_width[i];
                            let mz_lower = center - width / 2.0;
                            let mz_upper = center + width / 2.0;
                            // derive im bounds from scan starts/ends if present
                            let (im_lower, im_upper) = if i < quad.scan_starts.len() && i < quad.scan_ends.len() {
                                let start = quad.scan_starts[i] as f64;
                                let end = quad.scan_ends[i] as f64;
                                let im_start = im_converter.convert(start);
                                let im_end = im_converter.convert(end);
                                // start scans correspond to higher IM (domain-specific), so use min/max
                                (im_start.min(im_end), im_start.max(im_end))
                            } else {
                                (metadata.lower_im, metadata.upper_im)
                            };
                            out.push(TimsFfiSwathWindow {
                                mz_lower,
                                mz_upper,
                                mz_center: center,
                                im_lower,
                                im_upper,
                                is_ms1: 0,
                            });
                        }
                    }
                }
            }
            if out.is_empty() { None } else { Some(out) }
        };

        #[cfg(not(feature = "with_timsrust"))]
        let swath_windows = None;

        Ok(TimsDataset {
            reader,
            mz_buf: Vec::new(),
            int_buf: Vec::new(),
            swath_windows,
            last_error: None,
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
        if let Some(ref v) = self.swath_windows {
            return Ok(v.clone());
        }

        // If not available (feature disabled or metadata missing) return a
        // single wide window covering the full mz range as a reasonable fallback.
        #[cfg(feature = "with_timsrust")]
        {
            if let Ok(metadata) = timsrust::readers::MetadataReader::new("") {
                let mw = TimsFfiSwathWindow {
                    mz_lower: metadata.lower_mz,
                    mz_upper: metadata.upper_mz,
                    mz_center: (metadata.lower_mz + metadata.upper_mz) / 2.0,
                    im_lower: metadata.lower_im,
                    im_upper: metadata.upper_im,
                    is_ms1: 0,
                };
                return Ok(vec![mw]);
            }
        }

        // No metadata available: return empty vector.
        Ok(vec![])
    }
}

