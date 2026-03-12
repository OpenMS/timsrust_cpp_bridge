// tests/ffi_frame.rs
//
// Tests for tims_get_frame, tims_get_frames_by_level, and
// tims_free_frame_array: null-handle guards, index-OOB on stub,
// batch edge cases, and free safety.
//
// Tests that call open_stub() are gated to stub-only builds.

mod common;
use common::*;
use std::ptr;

// ============================================================
// Single frame tests — universal
// ============================================================

#[test]
fn get_frame_null_handle_returns_internal() {
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(ptr::null_mut(), 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// Single frame tests — stub-only
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_frame_index_0_stub_returns_oob() {
    let handle = open_stub();
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INDEX_OOB);
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_frame_null_out_returns_internal() {
    let handle = open_stub();
    let status = unsafe { tims_get_frame(handle, 0, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}

// ============================================================
// Batch frame tests — universal
// ============================================================

#[test]
fn get_frames_by_level_null_handle_returns_internal() {
    let mut count: u32 = 0;
    let mut frames: *mut TimsFfiFrame = ptr::null_mut();
    let status = unsafe { tims_get_frames_by_level(ptr::null_mut(), 1, &mut count, &mut frames) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

// ============================================================
// Batch frame tests — stub-only
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn get_frames_by_level_stub_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut frames: *mut TimsFfiFrame = ptr::null_mut();
    let status = unsafe { tims_get_frames_by_level(handle, 1, &mut count, &mut frames) };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0, "stub should return 0 frames");
    assert!(frames.is_null(), "frames pointer should be null when count is 0");
    unsafe { tims_close(handle) };
}

// ============================================================
// Free safety — stub-only
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn free_frame_array_null_no_crash() {
    let handle = open_stub();
    unsafe { tims_free_frame_array(handle, ptr::null_mut(), 0) };
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn free_frame_array_non_null_count_zero() {
    let handle = open_stub();
    // Allocate a small buffer via libc::malloc, pass count=0 so no
    // per-element iteration happens — only the outer array is freed.
    let buf = unsafe {
        libc::malloc(std::mem::size_of::<TimsFfiFrame>()) as *mut TimsFfiFrame
    };
    assert!(!buf.is_null(), "malloc should succeed");
    unsafe { tims_free_frame_array(handle, buf, 0) };
    unsafe { tims_close(handle) };
}
