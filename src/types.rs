// src/types.rs
use std::os::raw::{c_double, c_float, c_uint, c_uchar};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiSpectrum {
    pub rt_seconds: c_double,
    pub precursor_mz: c_double,
    pub ms_level: c_uchar,
    pub num_peaks: c_uint,
    pub mz: *const c_float,
    pub intensity: *const c_float,
    pub im: c_double,
    // New fields for Sage parity
    pub index: c_uint,               // Spectrum.index from SpectrumReader
    pub isolation_width: c_double,   // isolation window width (0.0 if N/A)
    pub isolation_mz: c_double,      // isolation window center m/z (0.0 if N/A)
    pub charge: c_uchar,             // precursor charge (0 = unknown)
    pub precursor_intensity: c_double, // precursor intensity (f64::NAN = unknown)
    pub frame_index: c_uint,         // precursor frame index (u32::MAX = N/A, i.e. MS1)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiSwathWindow {
    pub mz_lower: c_double,
    pub mz_upper: c_double,
    pub mz_center: c_double,
    pub im_lower: c_double,
    pub im_upper: c_double,
    pub is_ms1: c_uchar,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimsFfiStatus {
    Ok = 0,
    InvalidUtf8 = 1,
    OpenFailed = 2,
    IndexOutOfBounds = 3,
    Internal = 255,
}

// Helper: max length we'll copy for error strings via FFI
pub const TIMSFFI_MAX_ERROR_LEN: usize = 1024;

/// Per-MS-level statistics returned by tims_file_info.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiLevelStats {
    pub count: c_uint,          // number of spectra at this level
    pub total_peaks: u64,
    pub rt_min: c_double,
    pub rt_max: c_double,
    pub mz_min: c_double,
    pub mz_max: c_double,
    pub im_min: c_double,
    pub im_max: c_double,
    pub intensity_min: c_double,
    pub intensity_max: c_double,
}

impl TimsFfiLevelStats {
    pub fn empty() -> Self {
        TimsFfiLevelStats {
            count: 0,
            total_peaks: 0,
            rt_min: f64::INFINITY,
            rt_max: f64::NEG_INFINITY,
            mz_min: f64::INFINITY,
            mz_max: f64::NEG_INFINITY,
            im_min: f64::INFINITY,
            im_max: f64::NEG_INFINITY,
            intensity_min: f64::INFINITY,
            intensity_max: f64::NEG_INFINITY,
        }
    }
    pub fn finalize(&mut self) {
        // replace infinities with 0 for empty levels
        if self.rt_min.is_infinite()        { self.rt_min = 0.0; }
        if self.rt_max.is_infinite()        { self.rt_max = 0.0; }
        if self.mz_min.is_infinite()        { self.mz_min = 0.0; }
        if self.mz_max.is_infinite()        { self.mz_max = 0.0; }
        if self.im_min.is_infinite()        { self.im_min = 0.0; }
        if self.im_max.is_infinite()        { self.im_max = 0.0; }
        if self.intensity_min.is_infinite() { self.intensity_min = 0.0; }
        if self.intensity_max.is_infinite() { self.intensity_max = 0.0; }
    }
}

/// Aggregate file statistics returned by tims_file_info.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiFileInfo {
    pub num_frames: c_uint,      // total raw LC frames
    pub num_spectra_ms2: c_uint, // expanded DIA/DDA MS2 spectra
    pub total_peaks: u64,
    pub ms1: TimsFfiLevelStats,
    pub ms2: TimsFfiLevelStats,
    pub wall_ms: c_double,       // wall time to collect stats (ms)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiFrame {
    pub index: c_uint,
    pub rt_seconds: c_double,
    pub ms_level: c_uchar,        // 1=MS1, 2=MS2, 0=Unknown
    pub num_scans: c_uint,
    pub num_peaks: c_uint,         // total peaks (length of tof_indices & intensities)
    pub tof_indices: *const u32,   // raw TOF indices, flat array
    pub intensities: *const u32,   // raw intensities, flat array
    pub scan_offsets: *const u64,  // per-scan offsets (length: num_scans + 1)
}

