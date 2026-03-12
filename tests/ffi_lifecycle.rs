// tests/ffi_lifecycle.rs
//
// Lifecycle tests: tims_open / tims_close, open_with_config, config builder.

mod common;
use common::*;
use std::ffi::CString;
use std::ptr;

// ============================================================
// tims_open / tims_close tests
// ============================================================

#[test]
fn open_null_path_returns_internal() {
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(ptr::null(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn open_null_out_handle_returns_internal() {
    let c_path = CString::new("/tmp/whatever").unwrap();
    let status = unsafe { tims_open(c_path.as_ptr(), ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn open_nonexistent_path_returns_open_failed() {
    let c_path = CString::new("/tmp/timsrust_ffi_test_nonexistent_path_xyz").unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(c_path.as_ptr(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);
}

#[test]
fn open_valid_stub_succeeds() {
    let handle = open_stub();
    assert!(!handle.is_null());
    unsafe { tims_close(handle) };
}

#[test]
fn close_null_handle_no_crash() {
    unsafe { tims_close(ptr::null_mut()) };
}

#[test]
fn close_valid_handle_no_crash() {
    let handle = open_stub();
    unsafe { tims_close(handle) };
}

// ============================================================
// open_with_config error paths
// ============================================================

#[test]
fn open_with_config_null_path_returns_internal() {
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open_with_config(ptr::null(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_config_free(cfg) };
}

#[test]
fn open_with_config_null_config_returns_internal() {
    let c_path = CString::new("/tmp/whatever").unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open_with_config(c_path.as_ptr(), ptr::null(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn open_with_config_null_out_handle_returns_internal() {
    let c_path = CString::new("/tmp/whatever").unwrap();
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());
    let status = unsafe { tims_open_with_config(c_path.as_ptr(), cfg, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_config_free(cfg) };
}

#[test]
fn open_with_config_nonexistent_path_returns_open_failed() {
    let c_path = CString::new("/tmp/timsrust_ffi_test_nonexistent_path_xyz").unwrap();
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open_with_config(c_path.as_ptr(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);
    unsafe { tims_config_free(cfg) };
}

// ============================================================
// open_with_config happy path
// ============================================================

#[test]
fn open_with_config_happy_path() {
    let dir = std::env::temp_dir().join("timsrust_ffi_test_config_happy");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let c_path = CString::new(dir.to_str().unwrap()).unwrap();

    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());

    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open_with_config(c_path.as_ptr(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_OK);
    assert!(!handle.is_null());

    unsafe { tims_close(handle) };
    unsafe { tims_config_free(cfg) };
}

// ============================================================
// Config builder
// ============================================================

#[test]
fn config_create_returns_non_null() {
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());
    unsafe { tims_config_free(cfg) };
}

#[test]
fn config_free_null_no_crash() {
    unsafe { tims_config_free(ptr::null_mut()) };
}

#[test]
fn config_free_valid_no_crash() {
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());
    unsafe { tims_config_free(cfg) };
}

#[test]
fn config_setters_null_no_crash() {
    unsafe {
        tims_config_set_smoothing_window(ptr::null_mut(), 5);
        tims_config_set_centroiding_window(ptr::null_mut(), 3);
        tims_config_set_calibration_tolerance(ptr::null_mut(), 0.01);
        tims_config_set_calibrate(ptr::null_mut(), 1);
    }
}

#[test]
fn config_builder_full_lifecycle() {
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());

    unsafe {
        tims_config_set_smoothing_window(cfg, 10);
        tims_config_set_centroiding_window(cfg, 5);
        tims_config_set_calibration_tolerance(cfg, 0.05);
        tims_config_set_calibrate(cfg, 1);
    }

    let dir = std::env::temp_dir().join("timsrust_ffi_test_config_full");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let c_path = CString::new(dir.to_str().unwrap()).unwrap();

    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open_with_config(c_path.as_ptr(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_OK);
    assert!(!handle.is_null());

    unsafe { tims_close(handle) };
    unsafe { tims_config_free(cfg) };
}
