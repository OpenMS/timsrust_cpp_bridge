// tests/ffi_query.rs
//
// Tests for query/metadata functions: tims_num_spectra, tims_num_frames,
// tims_get_swath_windows, tims_free_swath_windows, and tims_file_info.

mod common;
use common::*;
use std::ptr;

// ============================================================
// tims_num_spectra
// ============================================================

#[test]
fn num_spectra_stub_returns_zero() {
    let handle = open_stub();
    let n = unsafe { tims_num_spectra(handle as *const _) };
    assert_eq!(n, 0, "stub dataset should have 0 spectra");
    unsafe { tims_close(handle) };
}

#[test]
fn num_spectra_null_handle_returns_zero() {
    let n = unsafe { tims_num_spectra(ptr::null()) };
    assert_eq!(n, 0, "null handle should return 0");
}

// ============================================================
// tims_num_frames
// ============================================================

#[test]
fn num_frames_stub_returns_zero() {
    let handle = open_stub();
    let n = unsafe { tims_num_frames(handle as *const _) };
    assert_eq!(n, 0, "stub dataset should have 0 frames");
    unsafe { tims_close(handle) };
}

#[test]
fn num_frames_null_handle_returns_zero() {
    let n = unsafe { tims_num_frames(ptr::null()) };
    assert_eq!(n, 0, "null handle should return 0");
}

// ============================================================
// tims_get_swath_windows
// ============================================================

#[test]
fn get_swath_windows_stub_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(handle, &mut count, &mut windows) };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0, "stub should have 0 swath windows");
    unsafe { tims_close(handle) };
}

#[test]
fn get_swath_windows_null_handle_returns_internal() {
    let mut count: u32 = 0;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(ptr::null_mut(), &mut count, &mut windows) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// tims_free_swath_windows
// ============================================================

#[test]
fn free_swath_windows_null_no_crash() {
    unsafe { tims_free_swath_windows(ptr::null_mut(), ptr::null_mut()) };
}

// ============================================================
// tims_file_info
// ============================================================

#[test]
fn file_info_stub_returns_zeros() {
    let handle = open_stub();
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };

    assert_eq!(info.num_frames, 0, "stub num_frames");
    assert_eq!(info.num_spectra_ms2, 0, "stub num_spectra_ms2");
    assert_eq!(info.total_peaks, 0, "stub total_peaks");
    assert_eq!(info.ms1.count, 0, "stub ms1.count");
    assert_eq!(info.ms2.count, 0, "stub ms2.count");
    assert_eq!(info.ms1.rt_min, 0.0, "stub ms1.rt_min");
    assert_eq!(info.ms1.rt_max, 0.0, "stub ms1.rt_max");
    assert_eq!(info.ms2.mz_min, 0.0, "stub ms2.mz_min");
    assert_eq!(info.ms2.mz_max, 0.0, "stub ms2.mz_max");

    unsafe { tims_close(handle) };
}

#[test]
fn file_info_null_handle_returns_internal() {
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(ptr::null_mut(), info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn file_info_null_out_returns_internal() {
    let handle = open_stub();
    let status = unsafe { tims_file_info(handle, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}
