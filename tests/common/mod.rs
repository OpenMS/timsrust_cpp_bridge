// tests/common/mod.rs
//
// Shared helpers for FFI integration tests.
// Uses extern "C" declarations to test the actual C ABI surface.

#![allow(dead_code)]

use std::ffi::CString;
use std::ptr;

// ---- FFI function declarations (matches include/timsrust_cpp_bridge.h) ----

extern "C" {
    pub fn tims_open(path: *const libc::c_char, out: *mut *mut libc::c_void) -> i32;
    pub fn tims_close(handle: *mut libc::c_void);
    pub fn tims_open_with_config(
        path: *const libc::c_char,
        cfg: *const libc::c_void,
        out: *mut *mut libc::c_void,
    ) -> i32;
    pub fn tims_num_spectra(handle: *const libc::c_void) -> u32;
    pub fn tims_num_frames(handle: *const libc::c_void) -> u32;
    pub fn tims_get_spectrum(
        handle: *mut libc::c_void,
        index: u32,
        out_spec: *mut TimsFfiSpectrum,
    ) -> i32;
    pub fn tims_get_spectra_by_rt(
        handle: *mut libc::c_void,
        rt_seconds: f64,
        n_spectra: i32,
        drift_start: f64,
        drift_end: f64,
        out_count: *mut u32,
        out_specs: *mut *mut TimsFfiSpectrum,
    ) -> i32;
    pub fn tims_free_spectrum_array(
        handle: *mut libc::c_void,
        specs: *mut TimsFfiSpectrum,
        count: u32,
    );
    pub fn tims_get_frame(
        handle: *mut libc::c_void,
        index: u32,
        out_frame: *mut TimsFfiFrame,
    ) -> i32;
    pub fn tims_get_frames_by_level(
        handle: *mut libc::c_void,
        ms_level: u8,
        out_count: *mut u32,
        out_frames: *mut *mut TimsFfiFrame,
    ) -> i32;
    pub fn tims_free_frame_array(
        handle: *mut libc::c_void,
        frames: *mut TimsFfiFrame,
        count: u32,
    );
    pub fn tims_get_swath_windows(
        handle: *mut libc::c_void,
        out_count: *mut u32,
        out_windows: *mut *mut TimsFfiSwathWindow,
    ) -> i32;
    pub fn tims_free_swath_windows(
        handle: *mut libc::c_void,
        windows: *mut TimsFfiSwathWindow,
    );
    pub fn tims_get_last_error(
        handle: *mut libc::c_void,
        buf: *mut libc::c_char,
        buf_len: u32,
    ) -> i32;
    pub fn tims_file_info(
        handle: *mut libc::c_void,
        out: *mut TimsFfiFileInfo,
    ) -> i32;
    pub fn tims_convert_tof_to_mz(handle: *const libc::c_void, tof_index: u32) -> f64;
    pub fn tims_convert_scan_to_im(handle: *const libc::c_void, scan_index: u32) -> f64;
    pub fn tims_convert_tof_to_mz_array(
        handle: *const libc::c_void,
        tof_indices: *const u32,
        count: u32,
        out_mz: *mut f64,
    ) -> i32;
    pub fn tims_convert_scan_to_im_array(
        handle: *const libc::c_void,
        scan_indices: *const u32,
        count: u32,
        out_im: *mut f64,
    ) -> i32;
    pub fn tims_config_create() -> *mut libc::c_void;
    pub fn tims_config_free(cfg: *mut libc::c_void);
    pub fn tims_config_set_smoothing_window(cfg: *mut libc::c_void, window: u32);
    pub fn tims_config_set_centroiding_window(cfg: *mut libc::c_void, window: u32);
    pub fn tims_config_set_calibration_tolerance(cfg: *mut libc::c_void, tolerance: f64);
    pub fn tims_config_set_calibrate(cfg: *mut libc::c_void, enabled: u8);
}

// ---- C-compatible struct mirrors (must match #[repr(C)] in src/types.rs) ----

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiSpectrum {
    pub rt_seconds: f64,
    pub precursor_mz: f64,
    pub ms_level: u8,
    pub num_peaks: u32,
    pub mz: *const f32,
    pub intensity: *const f32,
    pub im: f64,
    pub index: u32,
    pub isolation_width: f64,
    pub isolation_mz: f64,
    pub charge: u8,
    pub precursor_intensity: f64,
    pub frame_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiSwathWindow {
    pub mz_lower: f64,
    pub mz_upper: f64,
    pub mz_center: f64,
    pub im_lower: f64,
    pub im_upper: f64,
    pub is_ms1: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiLevelStats {
    pub count: u32,
    pub total_peaks: u64,
    pub rt_min: f64,
    pub rt_max: f64,
    pub mz_min: f64,
    pub mz_max: f64,
    pub im_min: f64,
    pub im_max: f64,
    pub intensity_min: f64,
    pub intensity_max: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiFileInfo {
    pub num_frames: u32,
    pub num_spectra_ms2: u32,
    pub total_peaks: u64,
    pub ms1: TimsFfiLevelStats,
    pub ms2: TimsFfiLevelStats,
    pub wall_ms: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiFrame {
    pub index: u32,
    pub rt_seconds: f64,
    pub ms_level: u8,
    pub num_scans: u32,
    pub num_peaks: u32,
    pub tof_indices: *const u32,
    pub intensities: *const u32,
    pub scan_offsets: *const u64,
}

// ---- Status codes (must match TimsFfiStatus enum in src/types.rs) ----
// Note: We use i32 constants rather than a Rust enum because the FFI
// functions are declared with `-> i32` to match the C ABI (the C enum
// `timsffi_status` is transmitted as a plain `int`).  This mirrors how
// a real C/C++ consumer would interpret the return values.

pub const TIMSFFI_OK: i32 = 0;
pub const TIMSFFI_ERR_INVALID_UTF8: i32 = 1;
pub const TIMSFFI_ERR_OPEN_FAILED: i32 = 2;
pub const TIMSFFI_ERR_INDEX_OOB: i32 = 3;
pub const TIMSFFI_ERR_INTERNAL: i32 = 255;

// ---- Helpers ----

/// Open a dataset against the stub build. Creates a temp directory
/// that exists on disk (stub open succeeds for any existing path).
pub fn open_stub() -> *mut libc::c_void {
    let dir = std::env::temp_dir().join("timsrust_ffi_test_stub");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let c_path = CString::new(dir.to_str().unwrap()).unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(c_path.as_ptr(), &mut handle) };
    assert_eq!(status, TIMSFFI_OK, "open_stub failed with status {status}");
    assert!(!handle.is_null());
    handle
}

/// Assert that a status code matches expected.
pub fn assert_status(actual: i32, expected: i32) {
    assert_eq!(
        actual, expected,
        "expected status {expected}, got {actual}"
    );
}

/// Returns DDA dataset path from env, or None.
pub fn dda_path() -> Option<String> {
    std::env::var("TIMSRUST_TEST_DATA_DDA").ok()
}

/// Returns DIA dataset path from env, or None.
pub fn dia_path() -> Option<String> {
    std::env::var("TIMSRUST_TEST_DATA_DIA").ok()
}

/// Open a real dataset by path. Panics on failure.
pub fn open_real(path: &str) -> *mut libc::c_void {
    let c_path = CString::new(path).unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(c_path.as_ptr(), &mut handle) };
    assert_eq!(status, TIMSFFI_OK, "open_real failed for {path}");
    assert!(!handle.is_null());
    handle
}
