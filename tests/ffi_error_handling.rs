// tests/ffi_error_handling.rs
//
// Tests for tims_get_last_error: global vs per-handle error retrieval,
// null/zero-length buffer edge cases, truncation, and message content.
//
// Tests that read global error state (null handle) share a process-wide
// Mutex to prevent races when the test runner executes in parallel.

mod common;
use common::*;
use std::ffi::CString;
use std::ptr;
use std::sync::Mutex;

/// Serialises tests that depend on the global LAST_ERROR state.
static GLOBAL_ERROR_LOCK: Mutex<()> = Mutex::new(());

// ============================================================
// 1. Global error via null handle
// ============================================================

#[test]
fn get_last_error_null_handle_reads_global() {
    let _lock = GLOBAL_ERROR_LOCK.lock().unwrap();

    // Trigger a global error by opening a nonexistent path.
    let bad = CString::new("/tmp/timsrust_ffi_test_does_not_exist_err1").unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(bad.as_ptr(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);

    // Read the global error (null handle → global).
    let mut buf = [0i8; 256];
    let st = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    assert_status(st, TIMSFFI_OK);

    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    let msg_str = msg.to_str().expect("valid utf-8");
    assert!(!msg_str.is_empty(), "global error message should be non-empty");
}

// ============================================================
// 2. Per-handle error via valid handle (stub-only)
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_last_error_with_valid_handle_reads_per_handle() {
    let handle = open_stub();

    // tims_get_frame with index 0 on stub (0 frames) → INDEX_OOB and sets
    // per-handle last_error.
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INDEX_OOB);

    // Read per-handle error.
    let mut buf = [0i8; 256];
    let st = unsafe { tims_get_last_error(handle, buf.as_mut_ptr(), buf.len() as u32) };
    assert_status(st, TIMSFFI_OK);

    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    let msg_str = msg.to_str().expect("valid utf-8");
    assert!(!msg_str.is_empty(), "per-handle error message should be non-empty");

    unsafe { tims_close(handle) };
}

// ============================================================
// 3. Null buffer pointer → INTERNAL
// ============================================================

#[test]
fn get_last_error_null_buffer_returns_internal() {
    let st = unsafe { tims_get_last_error(ptr::null_mut(), ptr::null_mut(), 128) };
    assert_status(st, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// 4. Zero-length buffer → INTERNAL
// ============================================================

#[test]
fn get_last_error_zero_length_buffer_returns_internal() {
    let mut buf = [0i8; 1];
    let st = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 0) };
    assert_status(st, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// 5. Truncation into a 5-byte buffer
// ============================================================

#[test]
fn get_last_error_truncation() {
    let _lock = GLOBAL_ERROR_LOCK.lock().unwrap();

    // Trigger a global error (message will be longer than 4 chars).
    let bad = CString::new("/tmp/timsrust_ffi_test_does_not_exist_err5").unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(bad.as_ptr(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);

    // Read into a 5-byte buffer → 4 chars + null terminator.
    let mut buf = [0i8; 5];
    let st = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 5) };
    assert_status(st, TIMSFFI_OK);

    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    let msg_str = msg.to_str().expect("valid utf-8");
    assert_eq!(msg_str.len(), 4, "truncated message should be exactly 4 chars, got '{}'", msg_str);
    // Ensure the null terminator is in the right place.
    assert_eq!(buf[4], 0, "5th byte should be null terminator");
}

// ============================================================
// 6. After successful open, global error is cleared (stub-only)
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_last_error_after_success_is_empty() {
    let _lock = GLOBAL_ERROR_LOCK.lock().unwrap();

    // open_stub() succeeds, which clears the global error.
    let handle = open_stub();

    // Read global error (null handle).
    let mut buf = [0i8; 256];
    let st = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    assert_status(st, TIMSFFI_OK);

    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    let msg_str = msg.to_str().expect("valid utf-8");
    assert!(msg_str.is_empty(), "global error should be empty after successful open, got '{}'", msg_str);

    unsafe { tims_close(handle) };
}

// ============================================================
// 7. Error message content is meaningful
// ============================================================

#[test]
fn error_message_content_meaningful() {
    let _lock = GLOBAL_ERROR_LOCK.lock().unwrap();

    let bad = CString::new("/tmp/timsrust_ffi_test_does_not_exist_err7").unwrap();
    let mut handle: *mut libc::c_void = ptr::null_mut();
    let status = unsafe { tims_open(bad.as_ptr(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);

    let mut buf = [0i8; 512];
    let st = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    assert_status(st, TIMSFFI_OK);

    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    let msg_str = msg.to_str().expect("valid utf-8").to_lowercase();
    let has_keyword = msg_str.contains("path")
        || msg_str.contains("not found")
        || msg_str.contains("error");
    assert!(
        has_keyword,
        "error message should contain 'path', 'not found', or 'error', got: '{}'",
        msg_str,
    );
}
