// tests/ffi_spectrum.rs
//
// Tests for tims_get_spectrum, tims_get_spectra_by_rt, and
// tims_free_spectrum_array: null-handle guards, index-OOB on stub,
// batch edge cases, and free safety.
//
// Tests that call open_stub() are gated to stub-only builds.

mod common;
use common::*;
use std::ptr;

// ============================================================
// Single spectrum tests — universal (no handle needed)
// ============================================================

#[test]
fn get_spectrum_null_handle_returns_internal() {
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(ptr::null_mut(), 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// Single spectrum tests — stub-only
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_spectrum_index_0_stub_returns_oob() {
    let handle = open_stub();
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INDEX_OOB);
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_spectrum_null_out_returns_internal() {
    let handle = open_stub();
    let status = unsafe { tims_get_spectrum(handle, 0, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}

// ============================================================
// Batch spectrum tests — universal
// ============================================================

#[test]
fn get_spectra_by_rt_null_handle_returns_internal() {
    let mut count: u32 = 0;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(ptr::null_mut(), 100.0, 5, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// Batch spectrum tests — stub-only
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_spectra_by_rt_stub_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, 100.0, 5, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0, "stub should return 0 spectra");
    assert!(specs.is_null(), "specs pointer should be null when count is 0");
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_spectra_by_rt_n_zero_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, 100.0, 0, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0, "n_spectra=0 should yield count=0");
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_spectra_by_rt_negative_n_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, 100.0, -5, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0, "negative n_spectra should yield count=0");
    unsafe { tims_close(handle) };
}

// ============================================================
// Free safety — stub-only
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn free_spectrum_array_null_no_crash() {
    let handle = open_stub();
    unsafe { tims_free_spectrum_array(handle, ptr::null_mut(), 0) };
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn free_spectrum_array_non_null_count_zero() {
    let handle = open_stub();
    // Allocate a small buffer via libc::malloc, pass count=0 so no
    // per-element iteration happens — only the outer array is freed.
    let buf = unsafe {
        libc::malloc(std::mem::size_of::<TimsFfiSpectrum>()) as *mut TimsFfiSpectrum
    };
    assert!(!buf.is_null(), "malloc should succeed");
    unsafe { tims_free_spectrum_array(handle, buf, 0) };
    unsafe { tims_close(handle) };
}
