// tests/ffi_converters.rs
//
// Tests for scalar and array index converters:
// tims_convert_tof_to_mz, tims_convert_scan_to_im,
// tims_convert_tof_to_mz_array, tims_convert_scan_to_im_array.

mod common;
use common::*;
use std::ptr;

// ============================================================
// Scalar converters — null handle
// ============================================================

#[test]
fn tof_to_mz_null_handle_returns_nan() {
    let result = unsafe { tims_convert_tof_to_mz(ptr::null(), 100) };
    assert!(result.is_nan(), "null handle should return NaN");
}

#[test]
fn scan_to_im_null_handle_returns_nan() {
    let result = unsafe { tims_convert_scan_to_im(ptr::null(), 100) };
    assert!(result.is_nan(), "null handle should return NaN");
}

// ============================================================
// Scalar converters — stub identity (stub-only)
// ============================================================

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn tof_to_mz_stub_returns_identity() {
    let handle = open_stub();
    let result = unsafe { tims_convert_tof_to_mz(handle as *const _, 42) };
    assert_eq!(result, 42.0, "stub should return identity (42.0)");
    unsafe { tims_close(handle) };
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn scan_to_im_stub_returns_identity() {
    let handle = open_stub();
    let result = unsafe { tims_convert_scan_to_im(handle as *const _, 99) };
    assert_eq!(result, 99.0, "stub should return identity (99.0)");
    unsafe { tims_close(handle) };
}

// ============================================================
// tims_convert_tof_to_mz_array — null checks
// ============================================================

#[test]
fn tof_to_mz_array_null_handle_returns_internal() {
    let input: [u32; 1] = [10];
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_tof_to_mz_array(ptr::null(), input.as_ptr(), 1, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn tof_to_mz_array_null_input_returns_internal() {
    let handle = open_stub();
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, ptr::null(), 1, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}

#[test]
fn tof_to_mz_array_null_output_returns_internal() {
    let handle = open_stub();
    let input: [u32; 1] = [10];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, input.as_ptr(), 1, ptr::null_mut())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}

#[test]
fn tof_to_mz_array_count_zero_returns_ok() {
    let handle = open_stub();
    let input: [u32; 1] = [10];
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, input.as_ptr(), 0, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(output[0], 0.0, "output should be untouched when count=0");
    unsafe { tims_close(handle) };
}

#[test]
fn tof_to_mz_array_stub_matches_scalar() {
    let handle = open_stub();
    let input: [u32; 3] = [10, 200, 5000];
    let mut output: [f64; 3] = [0.0; 3];

    let status = unsafe {
        tims_convert_tof_to_mz_array(
            handle as *const _,
            input.as_ptr(),
            input.len() as u32,
            output.as_mut_ptr(),
        )
    };
    assert_status(status, TIMSFFI_OK);

    for (i, &tof) in input.iter().enumerate() {
        let scalar = unsafe { tims_convert_tof_to_mz(handle as *const _, tof) };
        assert_eq!(
            output[i], scalar,
            "array[{i}] ({}) should match scalar ({scalar}) for tof={tof}",
            output[i],
        );
    }

    unsafe { tims_close(handle) };
}

// ============================================================
// tims_convert_scan_to_im_array — null checks
// ============================================================

#[test]
fn scan_to_im_array_null_handle_returns_internal() {
    let input: [u32; 1] = [10];
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_scan_to_im_array(ptr::null(), input.as_ptr(), 1, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn scan_to_im_array_null_input_returns_internal() {
    let handle = open_stub();
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_scan_to_im_array(handle as *const _, ptr::null(), 1, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}

#[test]
fn scan_to_im_array_null_output_returns_internal() {
    let handle = open_stub();
    let input: [u32; 1] = [10];
    let status = unsafe {
        tims_convert_scan_to_im_array(handle as *const _, input.as_ptr(), 1, ptr::null_mut())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle) };
}

#[test]
fn scan_to_im_array_count_zero_returns_ok() {
    let handle = open_stub();
    let input: [u32; 1] = [10];
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_scan_to_im_array(handle as *const _, input.as_ptr(), 0, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(output[0], 0.0, "output should be untouched when count=0");
    unsafe { tims_close(handle) };
}

#[test]
fn scan_to_im_array_stub_matches_scalar() {
    let handle = open_stub();
    let input: [u32; 3] = [10, 200, 5000];
    let mut output: [f64; 3] = [0.0; 3];

    let status = unsafe {
        tims_convert_scan_to_im_array(
            handle as *const _,
            input.as_ptr(),
            input.len() as u32,
            output.as_mut_ptr(),
        )
    };
    assert_status(status, TIMSFFI_OK);

    for (i, &scan) in input.iter().enumerate() {
        let scalar = unsafe { tims_convert_scan_to_im(handle as *const _, scan) };
        assert_eq!(
            output[i], scalar,
            "array[{i}] ({}) should match scalar ({scalar}) for scan={scan}",
            output[i],
        );
    }

    unsafe { tims_close(handle) };
}
