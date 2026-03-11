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
    // Total raw LC frame count (MS1 + MS2); 0 if not available
    num_frames: u32,
    // RT index: sorted (rt_seconds, spectrum_index) pairs for fast lookup
    rt_index: Vec<(f64, usize)>,
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
        let (swath_windows, num_frames) = {
            use timsrust::readers::QuadrupoleSettingsReader;
            use timsrust::readers::FrameReader;
            let mut out: Vec<TimsFfiSwathWindow> = Vec::new();
            let nf = FrameReader::new(path_str)
                .map(|fr| fr.len() as u32)
                .unwrap_or(0);
            if let Ok(quads) = QuadrupoleSettingsReader::new(path_str) {
                if let Ok(metadata) = timsrust::readers::MetadataReader::new(path_str) {
                    let im_converter = metadata.im_converter;
                    for quad in quads.iter() {
                        for i in 0..quad.len() {
                            let center = quad.isolation_mz[i];
                            let width = quad.isolation_width[i];
                            let mz_lower = center - width / 2.0;
                            let mz_upper = center + width / 2.0;
                            let (im_lower, im_upper) = if i < quad.scan_starts.len() && i < quad.scan_ends.len() {
                                let start = quad.scan_starts[i] as f64;
                                let end = quad.scan_ends[i] as f64;
                                let im_start = im_converter.convert(start);
                                let im_end = im_converter.convert(end);
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
            (if out.is_empty() { None } else { Some(out) }, nf)
        };

        #[cfg(not(feature = "with_timsrust"))]
        let (swath_windows, num_frames) = (None, 0u32);

        // Build RT index: cheaply read per-precursor RT without decompressing
        // peak data, so we can do fast nearest-RT lookups later.
        #[cfg(feature = "with_timsrust")]
        let rt_index = {
            use timsrust::readers::PrecursorReader;
            let mut idx: Vec<(f64, usize)> = Vec::new();
            if let Ok(prec_reader) = PrecursorReader::build()
                .with_path(path_str)
                .finalize()
            {
                let n = prec_reader.len();
                idx.reserve(n);
                for i in 0..n {
                    if let Some(p) = prec_reader.get(i) {
                        idx.push((p.rt, i));
                    }
                }
                idx.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }
            idx
        };

        #[cfg(not(feature = "with_timsrust"))]
        let rt_index: Vec<(f64, usize)> = Vec::new();

        Ok(TimsDataset {
            reader,
            mz_buf: Vec::new(),
            int_buf: Vec::new(),
            swath_windows,
            last_error: None,
            num_frames,
            rt_index,
        })
    }

    pub fn len(&self) -> u32 {
        self.reader.len() as u32
    }

    /// Number of raw LC frames (MS1 + MS2). Only available with timsrust.
    /// This counts all frames in the acquisition, not the expanded DIA spectra.
    pub fn num_frames(&self) -> u32 {
        self.num_frames
    }

    /// Sorted (rt_seconds, spectrum_index) pairs — built at open time from
    /// PrecursorReader without decompressing peaks. Used for fast RT lookup.
    pub(crate) fn rt_index(&self) -> &[(f64, usize)] {
        &self.rt_index
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
            let spec = self.reader.get(index as usize)
                .map_err(|_| TimsFfiStatus::IndexOutOfBounds)?;

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

            out.num_peaks = n as u32;
            out.mz = if n == 0 { ptr::null() } else { self.mz_buf.as_ptr() };
            out.intensity = if n == 0 { ptr::null() } else { self.int_buf.as_ptr() };
            out.index = spec.index as u32;
            out.isolation_width = spec.isolation_width;
            out.isolation_mz = spec.isolation_mz;

            if let Some(prec) = spec.precursor {
                out.rt_seconds = prec.rt;
                out.precursor_mz = prec.mz;
                out.im = prec.im;
                out.ms_level = 2;
                out.charge = prec.charge.map(|c| c as u8).unwrap_or(0);
                out.precursor_intensity = prec.intensity.unwrap_or(f64::NAN);
                out.frame_index = prec.frame_index as u32;
            } else {
                out.rt_seconds = 0.0;
                out.precursor_mz = 0.0;
                out.im = 0.0;
                out.ms_level = 1;
                out.charge = 0;
                out.precursor_intensity = f64::NAN;
                out.frame_index = u32::MAX;
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
            out.index = 0;
            out.isolation_width = 0.0;
            out.isolation_mz = 0.0;
            out.charge = 0;
            out.precursor_intensity = f64::NAN;
            out.frame_index = u32::MAX;
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

