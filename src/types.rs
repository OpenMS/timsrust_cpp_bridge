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

