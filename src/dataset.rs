// src/dataset.rs
use crate::types::{TimsFfiSpectrum, TimsFfiSwathWindow, TimsFfiFrame, TimsFfiStatus};
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
#[cfg(feature = "with_timsrust")]
use timsrust::readers::FrameReader;
#[cfg(feature = "with_timsrust")]
use timsrust::converters::{Tof2MzConverter, Scan2ImConverter};

#[cfg(not(feature = "with_timsrust"))]
pub(crate) struct SpectrumReader {
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

#[cfg(not(feature = "with_timsrust"))]
pub(crate) struct FrameReader { n: usize }

#[cfg(not(feature = "with_timsrust"))]
impl FrameReader {
    fn new(_path: &str) -> Result<Self, ()> { Ok(FrameReader { n: 0 }) }
    fn len(&self) -> usize { self.n }
}

#[cfg(not(feature = "with_timsrust"))]
pub(crate) struct Tof2MzConverter;
#[cfg(not(feature = "with_timsrust"))]
pub(crate) struct Scan2ImConverter;
#[cfg(not(feature = "with_timsrust"))]
impl Tof2MzConverter { pub fn convert(&self, value: f64) -> f64 { value } }
#[cfg(not(feature = "with_timsrust"))]
impl Scan2ImConverter { pub fn convert(&self, value: f64) -> f64 { value } }

pub struct TimsDataset {
    /// Spectrum-level reader (DDA/DIA expanded spectra).
    pub(crate) reader: SpectrumReader,
    /// Reusable buffer for spectrum m/z values (handle-owned).
    mz_buf: Vec<f32>,
    /// Reusable buffer for spectrum intensity values (handle-owned).
    int_buf: Vec<f32>,
    /// Reusable buffer for frame TOF indices (handle-owned, single-frame API).
    frame_tof_buf: Vec<u32>,
    /// Reusable buffer for frame intensities (handle-owned, single-frame API).
    frame_int_buf: Vec<u32>,
    /// Reusable buffer for frame scan offsets (handle-owned, single-frame API).
    frame_scan_offset_buf: Vec<u64>,
    /// Raw frame reader for MS1/MS2 frame-level access.
    pub(crate) frame_reader: FrameReader,
    /// TOF index → m/z converter, cached from MetadataReader at open time.
    pub(crate) mz_converter: Tof2MzConverter,
    /// Scan index → ion mobility converter, cached from MetadataReader at open time.
    pub(crate) im_converter: Scan2ImConverter,
    /// Precomputed DIA isolation windows (None if unavailable).
    swath_windows: Option<Vec<TimsFfiSwathWindow>>,
    /// Last error message for this handle (per-handle error storage).
    pub(crate) last_error: Option<String>,
    /// Sorted (rt_seconds, spectrum_index) pairs for fast RT lookup.
    rt_index: Vec<(f64, usize)>,
}

impl TimsDataset {
    pub fn open(path: &CStr) -> Result<Self, TimsFfiStatus> {
        let path_str = path.to_str().map_err(|_| TimsFfiStatus::InvalidUtf8)?;

        #[cfg(feature = "with_timsrust")]
        {
            let reader = SpectrumReader::new(path_str)
                .map_err(|_| TimsFfiStatus::OpenFailed)?;
            return Self::finish_open(path_str, reader);
        }

        #[cfg(not(feature = "with_timsrust"))]
        {
            let reader = SpectrumReader::new(path)
                .map_err(|_| TimsFfiStatus::OpenFailed)?;
            let fr = FrameReader::new(path_str).map_err(|_| TimsFfiStatus::OpenFailed)?;
            Ok(TimsDataset {
                reader,
                mz_buf: Vec::new(),
                int_buf: Vec::new(),
                frame_tof_buf: Vec::new(),
                frame_int_buf: Vec::new(),
                frame_scan_offset_buf: Vec::new(),
                frame_reader: fr,
                mz_converter: Tof2MzConverter,
                im_converter: Scan2ImConverter,
                swath_windows: None,
                last_error: None,
                rt_index: Vec::new(),
            })
        }
    }

    /// Common post-reader setup: reads metadata, builds swath windows, RT
    /// index, and frame reader. Only compiled with the real timsrust feature.
    #[cfg(feature = "with_timsrust")]
    fn finish_open(path_str: &str, reader: SpectrumReader) -> Result<Self, TimsFfiStatus> {
        use timsrust::readers::QuadrupoleSettingsReader;
        use timsrust::readers::PrecursorReader;

        let fr = FrameReader::new(path_str)
            .map_err(|_| TimsFfiStatus::OpenFailed)?;

        let metadata = MetadataReader::new(path_str)
            .map_err(|_| TimsFfiStatus::OpenFailed)?;
        let mz_conv = metadata.mz_converter;
        let im_conv = metadata.im_converter;

        let mut sw_out: Vec<TimsFfiSwathWindow> = Vec::new();
        if let Ok(quads) = QuadrupoleSettingsReader::new(path_str) {
            for quad in quads.iter() {
                for i in 0..quad.len() {
                    let center = quad.isolation_mz[i];
                    let width = quad.isolation_width[i];
                    let mz_lower = center - width / 2.0;
                    let mz_upper = center + width / 2.0;
                    let (im_lower, im_upper) = if i < quad.scan_starts.len() && i < quad.scan_ends.len() {
                        let start = quad.scan_starts[i] as f64;
                        let end = quad.scan_ends[i] as f64;
                        let im_start = im_conv.convert(start);
                        let im_end = im_conv.convert(end);
                        (im_start.min(im_end), im_start.max(im_end))
                    } else {
                        (metadata.lower_im, metadata.upper_im)
                    };
                    sw_out.push(TimsFfiSwathWindow {
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
        let swath_windows = if sw_out.is_empty() { None } else { Some(sw_out) };

        // Build RT index
        let mut rt_index: Vec<(f64, usize)> = Vec::new();
        if let Ok(prec_reader) = PrecursorReader::build()
            .with_path(path_str)
            .finalize()
        {
            let n = prec_reader.len();
            rt_index.reserve(n);
            for i in 0..n {
                if let Some(p) = prec_reader.get(i) {
                    rt_index.push((p.rt, i));
                }
            }
            rt_index.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        Ok(TimsDataset {
            reader,
            mz_buf: Vec::new(),
            int_buf: Vec::new(),
            frame_tof_buf: Vec::new(),
            frame_int_buf: Vec::new(),
            frame_scan_offset_buf: Vec::new(),
            frame_reader: fr,
            mz_converter: mz_conv,
            im_converter: im_conv,
            swath_windows,
            last_error: None,
            rt_index,
        })
    }

    /// Open a dataset with a custom SpectrumReaderConfig.
    #[cfg(feature = "with_timsrust")]
    pub fn open_with_config(path: &CStr, config: &crate::config::TimsFfiConfig) -> Result<Self, TimsFfiStatus> {
        let path_str = path.to_str().map_err(|_| TimsFfiStatus::InvalidUtf8)?;
        let reader = timsrust::readers::SpectrumReader::build()
            .with_path(path_str)
            .with_config(config.inner.clone())
            .finalize()
            .map_err(|_| TimsFfiStatus::OpenFailed)?;
        Self::finish_open(path_str, reader)
    }

    /// Stub: open_with_config delegates to open when timsrust is not enabled.
    #[cfg(not(feature = "with_timsrust"))]
    pub fn open_with_config(path: &CStr, _config: &crate::config::TimsFfiConfig) -> Result<Self, TimsFfiStatus> {
        Self::open(path)
    }

    pub fn len(&self) -> u32 {
        self.reader.len() as u32
    }

    /// Number of raw LC frames (MS1 + MS2). Only available with timsrust.
    /// This counts all frames in the acquisition, not the expanded DIA spectra.
    pub fn num_frames(&self) -> u32 {
        self.frame_reader.len() as u32
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

    pub fn get_frame(&mut self, index: u32, out: &mut TimsFfiFrame) -> Result<(), TimsFfiStatus> {
        let len = self.frame_reader.len() as u32;
        if index >= len {
            self.last_error = Some(format!(
                "frame index {} out of bounds (total {})", index, len
            ));
            return Err(TimsFfiStatus::IndexOutOfBounds);
        }

        #[cfg(feature = "with_timsrust")]
        {
            let frame = self.frame_reader.get(index as usize)
                .map_err(|e| {
                    self.last_error = Some(format!("failed to read frame {}: {:?}", index, e));
                    TimsFfiStatus::Internal
                })?;
            self.frame_tof_buf.clear();
            self.frame_tof_buf.extend_from_slice(&frame.tof_indices);
            self.frame_int_buf.clear();
            self.frame_int_buf.extend_from_slice(&frame.intensities);
            self.frame_scan_offset_buf.clear();
            self.frame_scan_offset_buf.extend(frame.scan_offsets.iter().map(|&s| s as u64));
            let num_scans = if frame.scan_offsets.is_empty() {
                0
            } else {
                (frame.scan_offsets.len() - 1) as u32
            };
            out.index = frame.index as u32;
            out.rt_seconds = frame.rt_in_seconds;
            out.ms_level = match frame.ms_level {
                timsrust::MSLevel::MS1 => 1,
                timsrust::MSLevel::MS2 => 2,
                _ => 0,
            };
            out.num_scans = num_scans;
            out.num_peaks = frame.tof_indices.len() as u32;
            out.tof_indices = if out.num_peaks == 0 { ptr::null() } else { self.frame_tof_buf.as_ptr() };
            out.intensities = if out.num_peaks == 0 { ptr::null() } else { self.frame_int_buf.as_ptr() };
            out.scan_offsets = if num_scans == 0 { ptr::null() } else { self.frame_scan_offset_buf.as_ptr() };
            self.last_error = None;
            return Ok(());
        }

        #[cfg(not(feature = "with_timsrust"))]
        {
            out.index = index;
            out.rt_seconds = 0.0;
            out.ms_level = 0;
            out.num_scans = 0;
            out.num_peaks = 0;
            out.tof_indices = ptr::null();
            out.intensities = ptr::null();
            out.scan_offsets = ptr::null();
            self.last_error = None;
            Ok(())
        }
    }
}

