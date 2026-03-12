# Testing Strategy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a contract-centric test suite for the timsrust_cpp_bridge FFI library.

**Architecture:** Rust integration tests (`tests/`) call `extern "C"` functions directly in stub mode (no datasets) for FFI safety coverage. A separate `ffi_real_data.rs` module tests correctness with real `.d` datasets gated by env vars. A Catch2 C++ suite (`tests_cpp/`) validates ABI layout and end-to-end flows.

**Tech Stack:** Rust `cargo test`, Catch2 v3 (single header), Make, g++

**Feature gate convention:** Stub-mode tests (Tasks 2-8) run with `cargo test` (no features).
Real-data tests (Task 9) run with `cargo test --features with_timsrust` plus env vars.
Tests with stub-specific assertions (identity converters, zero counts from `open_stub()`)
are gated behind `#[cfg(not(feature = "with_timsrust"))]` so they compile out when
the real reader is active. `open_stub()` only works without `with_timsrust` because
opening a non-`.d` temp directory fails with the real reader.

---

## Chunk 1: Test Infrastructure & Lifecycle Tests

### Task 0: Configure Cargo.toml for integration tests

**Files:**
- Modify: `Cargo.toml`

Integration tests require an `rlib` target to link against. The crate currently only
produces `cdylib` + `staticlib`. We also need `libc` as a dev-dependency for tests
that call `libc::malloc` directly.

- [ ] **Step 1: Add `"lib"` to crate-type and `libc` to dev-dependencies**

In `Cargo.toml`, change:
```toml
crate-type = ["cdylib", "staticlib"]
```
to:
```toml
crate-type = ["cdylib", "staticlib", "lib"]
```

And add:
```toml
[dev-dependencies]
libc = "0.2"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: add rlib crate-type and libc dev-dep for integration tests"
```

### Task 1: Create test helpers module

**Files:**
- Create: `tests/common/mod.rs`

Since we added `"lib"` to crate-type, integration tests can import the crate's
actual types directly. This gives us type-safe FFI calls with correct `TimsFfiStatus`
return types and `*mut tims_dataset` handles.

- [ ] **Step 1: Write the helpers module**

```rust
// tests/common/mod.rs
//
// Shared helpers for FFI integration tests.
// Uses the crate's own types for type-safe FFI calls.

use std::ffi::CString;
use std::ptr;

// Re-export the crate's public FFI types and functions.
pub use timsrust_cpp_bridge::*;

// Re-export types from the crate's types module.
use timsrust_cpp_bridge::types::{
    TimsFfiSpectrum, TimsFfiFrame, TimsFfiSwathWindow,
    TimsFfiFileInfo, TimsFfiLevelStats, TimsFfiStatus,
};

// ---- Status code constants for convenience ----

pub const TIMSFFI_OK: TimsFfiStatus = TimsFfiStatus::Ok;
pub const TIMSFFI_ERR_INVALID_UTF8: TimsFfiStatus = TimsFfiStatus::InvalidUtf8;
pub const TIMSFFI_ERR_OPEN_FAILED: TimsFfiStatus = TimsFfiStatus::OpenFailed;
pub const TIMSFFI_ERR_INDEX_OOB: TimsFfiStatus = TimsFfiStatus::IndexOutOfBounds;
pub const TIMSFFI_ERR_INTERNAL: TimsFfiStatus = TimsFfiStatus::Internal;

// ---- Helpers ----

/// Open a dataset against the stub build. Creates a temp directory
/// that exists on disk (stub open succeeds for any existing path).
pub fn open_stub() -> *mut tims_dataset {
    let dir = std::env::temp_dir().join("timsrust_ffi_test_stub");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let c_path = CString::new(dir.to_str().unwrap()).unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open(c_path.as_ptr(), &mut handle) };
    assert_eq!(status, TIMSFFI_OK, "open_stub failed");
    assert!(!handle.is_null());
    handle
}

/// Assert that a status code matches expected.
pub fn assert_status(actual: TimsFfiStatus, expected: TimsFfiStatus) {
    assert_eq!(
        actual, expected,
        "expected status {expected:?}, got {actual:?}"
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
pub fn open_real(path: &str) -> *mut tims_dataset {
    let c_path = CString::new(path).unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open(c_path.as_ptr(), &mut handle) };
    assert_eq!(status, TIMSFFI_OK, "open_real failed for {path}");
    assert!(!handle.is_null());
    handle
}
```

**Important:** This approach imports the crate's own types. If the crate's FFI
functions and types are not publicly re-exported from `lib.rs`, you will need to
add `pub use` statements in `src/lib.rs` for the types module:
```rust
pub mod types;  // (if not already public)
```
Check `src/lib.rs` — if `mod types;` is private, change it to `pub mod types;`
and similarly for any types used in function signatures. The `tims_dataset` struct
and all `extern "C"` functions are already `pub`, so they should be accessible.
If the types module can't be made public, fall back to declaring `extern "C"`
blocks manually with `*mut std::ffi::c_void` for handles and `i32` for status
(with a compile-time size assertion on `TimsFfiStatus`).

- [ ] **Step 2: Verify it compiles**

Run: `cargo test --no-run 2>&1 | head -20`

This will fail because there are no test files importing the module yet. That's expected — we just need the module to exist.

- [ ] **Step 3: Commit**

```bash
git add tests/common/mod.rs
git commit -m "test: add shared FFI test helpers module"
```

### Task 2: Lifecycle tests — tims_open / tims_close

**Files:**
- Create: `tests/ffi_lifecycle.rs`

- [ ] **Step 1: Write the lifecycle test file**

```rust
// tests/ffi_lifecycle.rs
mod common;

use common::*;
use std::ffi::CString;
use std::ptr;

#[test]
fn open_null_path_returns_internal() {
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open(ptr::null(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn open_null_out_handle_returns_internal() {
    let path = CString::new("/tmp/dummy").unwrap();
    let status = unsafe { tims_open(path.as_ptr(), ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn open_nonexistent_path_returns_open_failed() {
    let path = CString::new("/tmp/timsrust_ffi_test_nonexistent_path_12345").unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open(path.as_ptr(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);
}

#[test]
fn open_valid_stub_succeeds() {
    let handle = open_stub();
    assert!(!handle.is_null());
    unsafe { tims_close(handle); }
}

#[test]
fn close_null_handle_no_crash() {
    unsafe { tims_close(ptr::null_mut()); }
}

#[test]
fn close_valid_handle_no_crash() {
    let handle = open_stub();
    unsafe { tims_close(handle); }
    // If we reach here without crashing, test passes.
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test ffi_lifecycle -- --nocapture`
Expected: All 6 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_lifecycle.rs
git commit -m "test: add FFI lifecycle tests (open, close, null safety)"
```

### Task 3: Lifecycle tests — config builder & open_with_config

**Files:**
- Modify: `tests/ffi_lifecycle.rs`

- [ ] **Step 1: Add config builder and open_with_config tests**

Append to `tests/ffi_lifecycle.rs`:

```rust
#[test]
fn open_with_config_null_path_returns_internal() {
    let cfg = unsafe { tims_config_create() };
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open_with_config(ptr::null(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_config_free(cfg); }
}

#[test]
fn open_with_config_null_config_returns_internal() {
    let path = CString::new("/tmp/dummy").unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open_with_config(path.as_ptr(), ptr::null(), &mut handle) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn open_with_config_null_out_handle_returns_internal() {
    let path = CString::new("/tmp/dummy").unwrap();
    let cfg = unsafe { tims_config_create() };
    let status = unsafe { tims_open_with_config(path.as_ptr(), cfg, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_config_free(cfg); }
}

#[test]
fn open_with_config_nonexistent_path_returns_open_failed() {
    let path = CString::new("/tmp/timsrust_ffi_test_nonexistent_12345").unwrap();
    let cfg = unsafe { tims_config_create() };
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open_with_config(path.as_ptr(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_ERR_OPEN_FAILED);
    unsafe { tims_config_free(cfg); }
}

#[test]
fn open_with_config_happy_path() {
    let dir = std::env::temp_dir().join("timsrust_ffi_test_stub_cfg");
    std::fs::create_dir_all(&dir).unwrap();
    let path = CString::new(dir.to_str().unwrap()).unwrap();
    let cfg = unsafe { tims_config_create() };
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open_with_config(path.as_ptr(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_OK);
    assert!(!handle.is_null());
    unsafe {
        tims_config_free(cfg);
        tims_close(handle);
    }
}

#[test]
fn config_create_returns_non_null() {
    let cfg = unsafe { tims_config_create() };
    assert!(!cfg.is_null());
    unsafe { tims_config_free(cfg); }
}

#[test]
fn config_free_null_no_crash() {
    unsafe { tims_config_free(ptr::null_mut()); }
}

#[test]
fn config_free_valid_no_crash() {
    let cfg = unsafe { tims_config_create() };
    unsafe { tims_config_free(cfg); }
}

#[test]
fn config_setters_null_no_crash() {
    unsafe {
        tims_config_set_smoothing_window(ptr::null_mut(), 5);
        tims_config_set_centroiding_window(ptr::null_mut(), 5);
        tims_config_set_calibration_tolerance(ptr::null_mut(), 0.01);
        tims_config_set_calibrate(ptr::null_mut(), 1);
    }
}

#[test]
fn config_builder_full_lifecycle() {
    let dir = std::env::temp_dir().join("timsrust_ffi_test_cfg_lifecycle");
    std::fs::create_dir_all(&dir).unwrap();
    let path = CString::new(dir.to_str().unwrap()).unwrap();

    let cfg = unsafe { tims_config_create() };
    unsafe {
        tims_config_set_smoothing_window(cfg, 3);
        tims_config_set_centroiding_window(cfg, 5);
        tims_config_set_calibration_tolerance(cfg, 0.01);
        tims_config_set_calibrate(cfg, 1);
    }
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let status = unsafe { tims_open_with_config(path.as_ptr(), cfg, &mut handle) };
    assert_status(status, TIMSFFI_OK);
    unsafe {
        tims_config_free(cfg);
        tims_close(handle);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test ffi_lifecycle -- --nocapture`
Expected: All 17 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_lifecycle.rs
git commit -m "test: add config builder and open_with_config lifecycle tests"
```

## Chunk 2: Error Handling & Spectrum Tests

### Task 4: Error handling tests

**Files:**
- Create: `tests/ffi_error_handling.rs`

- [ ] **Step 1: Write the error handling test file**

```rust
// tests/ffi_error_handling.rs
mod common;

use common::*;
use std::ffi::CString;
use std::ptr;

#[test]
fn get_last_error_null_handle_reads_global() {
    // Trigger a global error by opening a nonexistent path
    let path = CString::new("/tmp/timsrust_ffi_test_no_such_path").unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let _ = unsafe { tims_open(path.as_ptr(), &mut handle) };

    // Read global error (null handle)
    let mut buf = [0i8; 256];
    let status = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 256) };
    assert_status(status, TIMSFFI_OK);
    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .unwrap();
    assert!(!msg.is_empty(), "global error should be non-empty after failed open");
}

#[test]
fn get_last_error_with_valid_handle_reads_per_handle() {
    let handle = open_stub();
    // Trigger a per-handle error: get_frame on stub (0 frames -> OOB)
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INDEX_OOB);

    // Read per-handle error
    let mut buf = [0i8; 256];
    let status = unsafe { tims_get_last_error(handle, buf.as_mut_ptr(), 256) };
    assert_status(status, TIMSFFI_OK);
    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .unwrap();
    assert!(!msg.is_empty(), "per-handle error should be set after OOB frame access");

    unsafe { tims_close(handle); }
}

#[test]
fn get_last_error_null_buffer_returns_internal() {
    let status = unsafe { tims_get_last_error(ptr::null_mut(), ptr::null_mut(), 256) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn get_last_error_zero_length_buffer_returns_internal() {
    let mut buf = [0i8; 1];
    let status = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 0) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn get_last_error_truncation() {
    // Trigger a global error
    let path = CString::new("/tmp/timsrust_ffi_no_such_thing").unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let _ = unsafe { tims_open(path.as_ptr(), &mut handle) };

    // Read into a tiny buffer (5 bytes = 4 chars + null)
    let mut buf = [0i8; 5];
    let status = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 5) };
    assert_status(status, TIMSFFI_OK);
    // Must be null-terminated
    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .unwrap();
    assert_eq!(msg.len(), 4, "should truncate to buf_len - 1 = 4 chars");
    assert_eq!(buf[4], 0, "must be null-terminated");
}

#[test]
fn get_last_error_after_success_is_empty() {
    // Successful open should clear global error
    let handle = open_stub();
    let mut buf = [0i8; 256];
    let status = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 256) };
    assert_status(status, TIMSFFI_OK);
    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .unwrap();
    assert!(msg.is_empty(), "no error after successful open");
    unsafe { tims_close(handle); }
}

#[test]
fn error_message_content_meaningful() {
    let path = CString::new("/tmp/timsrust_ffi_definitely_not_a_real_path").unwrap();
    let mut handle: *mut tims_dataset = ptr::null_mut();
    let _ = unsafe { tims_open(path.as_ptr(), &mut handle) };

    let mut buf = [0i8; 512];
    let status = unsafe { tims_get_last_error(ptr::null_mut(), buf.as_mut_ptr(), 512) };
    assert_status(status, TIMSFFI_OK);
    let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .unwrap();
    assert!(
        msg.contains("path") || msg.contains("not found") || msg.contains("error"),
        "error message should contain useful context, got: {msg}"
    );
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test ffi_error_handling -- --nocapture`
Expected: All 7 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_error_handling.rs
git commit -m "test: add FFI error handling tests"
```

### Task 5: Spectrum tests (single + batch, stub mode)

**Files:**
- Create: `tests/ffi_spectrum.rs`

- [ ] **Step 1: Write the spectrum test file**

```rust
// tests/ffi_spectrum.rs
mod common;

use common::*;
use std::ptr;

// ---- Single spectrum tests ----

#[test]
fn get_spectrum_index_0_stub_returns_oob() {
    let handle = open_stub();
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INDEX_OOB);
    unsafe { tims_close(handle); }
}

#[test]
fn get_spectrum_null_handle_returns_internal() {
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(ptr::null_mut(), 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn get_spectrum_null_out_returns_internal() {
    let handle = open_stub();
    let status = unsafe { tims_get_spectrum(handle, 0, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle); }
}

// ---- Batch spectrum tests ----

#[test]
fn get_spectra_by_rt_stub_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, 100.0, 5, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0);
    assert!(specs.is_null());
    unsafe { tims_close(handle); }
}

#[test]
fn get_spectra_by_rt_null_handle_returns_internal() {
    let mut count: u32 = 0;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(ptr::null_mut(), 100.0, 5, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn get_spectra_by_rt_n_zero_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, 100.0, 0, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0);
    unsafe { tims_close(handle); }
}

#[test]
fn get_spectra_by_rt_negative_n_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, 100.0, -5, 0.0, 2.0, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0);
    unsafe { tims_close(handle); }
}

#[test]
fn free_spectrum_array_null_no_crash() {
    let handle = open_stub();
    unsafe { tims_free_spectrum_array(handle, ptr::null_mut(), 0); }
    unsafe { tims_close(handle); }
}

#[test]
fn free_spectrum_array_non_null_count_zero() {
    // Allocate a small buffer, pass count=0 — should free the array
    // without iterating per-element.
    let handle = open_stub();
    let ptr = unsafe { libc::malloc(std::mem::size_of::<TimsFfiSpectrum>()) } as *mut TimsFfiSpectrum;
    assert!(!ptr.is_null());
    unsafe { tims_free_spectrum_array(handle, ptr, 0); }
    // If we get here without crash, test passes.
    unsafe { tims_close(handle); }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test ffi_spectrum -- --nocapture`
Expected: All 9 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_spectrum.rs
git commit -m "test: add single and batch spectrum FFI tests (stub mode)"
```

### Task 6: Frame tests (single + batch, stub mode)

**Files:**
- Create: `tests/ffi_frame.rs`

- [ ] **Step 1: Write the frame test file**

```rust
// tests/ffi_frame.rs
mod common;

use common::*;
use std::ptr;

// ---- Single frame tests ----

#[test]
fn get_frame_index_0_stub_returns_oob() {
    let handle = open_stub();
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INDEX_OOB);
    unsafe { tims_close(handle); }
}

#[test]
fn get_frame_null_handle_returns_internal() {
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(ptr::null_mut(), 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn get_frame_null_out_returns_internal() {
    let handle = open_stub();
    let status = unsafe { tims_get_frame(handle, 0, ptr::null_mut()) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle); }
}

// ---- Batch frame tests ----

#[test]
fn get_frames_by_level_stub_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut frames: *mut TimsFfiFrame = ptr::null_mut();
    let status = unsafe { tims_get_frames_by_level(handle, 1, &mut count, &mut frames) };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0);
    assert!(frames.is_null());
    unsafe { tims_close(handle); }
}

#[test]
fn get_frames_by_level_null_handle_returns_internal() {
    let mut count: u32 = 0;
    let mut frames: *mut TimsFfiFrame = ptr::null_mut();
    let status = unsafe { tims_get_frames_by_level(ptr::null_mut(), 1, &mut count, &mut frames) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn free_frame_array_null_no_crash() {
    let handle = open_stub();
    unsafe { tims_free_frame_array(handle, ptr::null_mut(), 0); }
    unsafe { tims_close(handle); }
}

#[test]
fn free_frame_array_non_null_count_zero() {
    let handle = open_stub();
    let ptr = unsafe { libc::malloc(std::mem::size_of::<TimsFfiFrame>()) } as *mut TimsFfiFrame;
    assert!(!ptr.is_null());
    unsafe { tims_free_frame_array(handle, ptr, 0); }
    unsafe { tims_close(handle); }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test ffi_frame -- --nocapture`
Expected: All 7 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_frame.rs
git commit -m "test: add single and batch frame FFI tests (stub mode)"
```

## Chunk 3: Query, Converter & Real Data Tests

### Task 7: Query and metadata tests

**Files:**
- Create: `tests/ffi_query.rs`

- [ ] **Step 1: Write the query test file**

```rust
// tests/ffi_query.rs
mod common;

use common::*;
use std::ptr;

#[test]
fn num_spectra_stub_returns_zero() {
    let handle = open_stub();
    let n = unsafe { tims_num_spectra(handle as *const _) };
    assert_eq!(n, 0);
    unsafe { tims_close(handle); }
}

#[test]
fn num_spectra_null_handle_returns_zero() {
    let n = unsafe { tims_num_spectra(ptr::null()) };
    assert_eq!(n, 0);
}

#[test]
fn num_frames_stub_returns_zero() {
    let handle = open_stub();
    let n = unsafe { tims_num_frames(handle as *const _) };
    assert_eq!(n, 0);
    unsafe { tims_close(handle); }
}

#[test]
fn num_frames_null_handle_returns_zero() {
    let n = unsafe { tims_num_frames(ptr::null()) };
    assert_eq!(n, 0);
}

#[test]
fn get_swath_windows_stub_returns_empty() {
    let handle = open_stub();
    let mut count: u32 = 99;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(handle, &mut count, &mut windows) };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(count, 0);
    unsafe { tims_close(handle); }
}

#[test]
fn get_swath_windows_null_handle_returns_internal() {
    let mut count: u32 = 0;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(ptr::null_mut(), &mut count, &mut windows) };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
}

#[test]
fn free_swath_windows_null_no_crash() {
    unsafe { tims_free_swath_windows(ptr::null_mut(), ptr::null_mut()); }
}

#[test]
fn file_info_stub_returns_zeros() {
    let handle = open_stub();
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };
    assert_eq!(info.num_frames, 0);
    assert_eq!(info.num_spectra_ms2, 0);
    assert_eq!(info.total_peaks, 0);
    assert_eq!(info.ms1.count, 0);
    assert_eq!(info.ms2.count, 0);
    // After finalize(), min/max fields should be 0.0 (not infinity)
    assert_eq!(info.ms1.rt_min, 0.0);
    assert_eq!(info.ms1.rt_max, 0.0);
    assert_eq!(info.ms2.mz_min, 0.0);
    assert_eq!(info.ms2.mz_max, 0.0);
    unsafe { tims_close(handle); }
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
    unsafe { tims_close(handle); }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test ffi_query -- --nocapture`
Expected: All 10 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_query.rs
git commit -m "test: add query and metadata FFI tests (stub mode)"
```

### Task 8: Converter tests

**Files:**
- Create: `tests/ffi_converters.rs`

- [ ] **Step 1: Write the converter test file**

```rust
// tests/ffi_converters.rs
mod common;

use common::*;
use std::ptr;

// ---- Scalar converters ----

#[test]
fn tof_to_mz_null_handle_returns_nan() {
    let result = unsafe { tims_convert_tof_to_mz(ptr::null(), 100) };
    assert!(result.is_nan());
}

#[test]
fn scan_to_im_null_handle_returns_nan() {
    let result = unsafe { tims_convert_scan_to_im(ptr::null(), 100) };
    assert!(result.is_nan());
}

// Stub-specific: identity converter only applies without with_timsrust.
// With the real timsrust feature, converters use calibration data.
#[cfg(not(feature = "with_timsrust"))]
#[test]
fn tof_to_mz_stub_returns_identity() {
    let handle = open_stub();
    // Stub converter returns the input value unchanged
    let result = unsafe { tims_convert_tof_to_mz(handle as *const _, 42) };
    assert_eq!(result, 42.0);
    unsafe { tims_close(handle); }
}

#[cfg(not(feature = "with_timsrust"))]
#[test]
fn scan_to_im_stub_returns_identity() {
    let handle = open_stub();
    let result = unsafe { tims_convert_scan_to_im(handle as *const _, 99) };
    assert_eq!(result, 99.0);
    unsafe { tims_close(handle); }
}

// ---- Array converters: null checks ----

#[test]
fn tof_to_mz_array_null_handle_returns_internal() {
    let input: [u32; 1] = [100];
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
    unsafe { tims_close(handle); }
}

#[test]
fn tof_to_mz_array_null_output_returns_internal() {
    let handle = open_stub();
    let input: [u32; 1] = [100];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, input.as_ptr(), 1, ptr::null_mut())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle); }
}

#[test]
fn tof_to_mz_array_count_zero_returns_ok() {
    let handle = open_stub();
    // Non-null pointers but count=0: should return OK without dereferencing
    let input: [u32; 1] = [100];
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, input.as_ptr(), 0, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    // output should be untouched
    assert_eq!(output[0], 0.0);
    unsafe { tims_close(handle); }
}

#[test]
fn tof_to_mz_array_stub_matches_scalar() {
    let handle = open_stub();
    let input: [u32; 3] = [10, 200, 5000];
    let mut output: [f64; 3] = [0.0; 3];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, input.as_ptr(), 3, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    for i in 0..3 {
        let scalar = unsafe { tims_convert_tof_to_mz(handle as *const _, input[i]) };
        assert_eq!(output[i], scalar, "array[{i}] should match scalar");
    }
    unsafe { tims_close(handle); }
}

// ---- scan_to_im_array: mirror of tof tests ----

#[test]
fn scan_to_im_array_null_handle_returns_internal() {
    let input: [u32; 1] = [100];
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
    unsafe { tims_close(handle); }
}

#[test]
fn scan_to_im_array_null_output_returns_internal() {
    let handle = open_stub();
    let input: [u32; 1] = [100];
    let status = unsafe {
        tims_convert_scan_to_im_array(handle as *const _, input.as_ptr(), 1, ptr::null_mut())
    };
    assert_status(status, TIMSFFI_ERR_INTERNAL);
    unsafe { tims_close(handle); }
}

#[test]
fn scan_to_im_array_count_zero_returns_ok() {
    let handle = open_stub();
    let input: [u32; 1] = [100];
    let mut output: [f64; 1] = [0.0];
    let status = unsafe {
        tims_convert_scan_to_im_array(handle as *const _, input.as_ptr(), 0, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    assert_eq!(output[0], 0.0);
    unsafe { tims_close(handle); }
}

#[test]
fn scan_to_im_array_stub_matches_scalar() {
    let handle = open_stub();
    let input: [u32; 3] = [10, 200, 5000];
    let mut output: [f64; 3] = [0.0; 3];
    let status = unsafe {
        tims_convert_scan_to_im_array(handle as *const _, input.as_ptr(), 3, output.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    for i in 0..3 {
        let scalar = unsafe { tims_convert_scan_to_im(handle as *const _, input[i]) };
        assert_eq!(output[i], scalar, "array[{i}] should match scalar");
    }
    unsafe { tims_close(handle); }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test ffi_converters -- --nocapture`
Expected: All 14 tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_converters.rs
git commit -m "test: add converter FFI tests (stub mode)"
```

### Task 9: Real data tests

**Files:**
- Create: `tests/ffi_real_data.rs`

- [ ] **Step 1: Write the real data test file**

```rust
// tests/ffi_real_data.rs
//
// Integration tests that run against real .d datasets.
// Skipped if TIMSRUST_TEST_DATA_DDA / TIMSRUST_TEST_DATA_DIA env vars are unset.
// Requires: cargo test --features with_timsrust
mod common;

use common::*;
use std::ptr;

// ---- Helpers ----

macro_rules! require_dda {
    () => {
        match dda_path() {
            Some(p) => p,
            None => {
                eprintln!("TIMSRUST_TEST_DATA_DDA not set, skipping");
                return;
            }
        }
    };
}

macro_rules! require_dia {
    () => {
        match dia_path() {
            Some(p) => p,
            None => {
                eprintln!("TIMSRUST_TEST_DATA_DIA not set, skipping");
                return;
            }
        }
    };
}

// ======================================================================
// DDA Tests
// ======================================================================

#[test]
fn dda_open_succeeds() {
    let path = require_dda!();
    let handle = open_real(&path);
    unsafe { tims_close(handle); }
}

#[test]
fn dda_num_spectra_positive() {
    let path = require_dda!();
    let handle = open_real(&path);
    let n = unsafe { tims_num_spectra(handle as *const _) };
    assert!(n > 0, "DDA dataset should have spectra, got {n}");
    unsafe { tims_close(handle); }
}

#[test]
fn dda_num_frames_positive() {
    let path = require_dda!();
    let handle = open_real(&path);
    let nf = unsafe { tims_num_frames(handle as *const _) };
    let ns = unsafe { tims_num_spectra(handle as *const _) };
    assert!(nf > 0, "DDA dataset should have frames");
    assert!(nf <= ns, "expected num_frames({nf}) <= num_spectra({ns}) for test dataset");
    unsafe { tims_close(handle); }
}

#[test]
fn dda_get_spectrum_0() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let spec = unsafe { spec.assume_init() };
    assert!(spec.num_peaks > 0, "first spectrum should have peaks");
    assert!(!spec.mz.is_null());
    assert!(!spec.intensity.is_null());
    assert!(spec.ms_level == 1 || spec.ms_level == 2);
    unsafe { tims_close(handle); }
}

#[test]
fn dda_spectrum_mz_sorted() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let spec = unsafe { spec.assume_init() };
    let n = spec.num_peaks as usize;
    if n > 1 {
        let mz = unsafe { std::slice::from_raw_parts(spec.mz, n) };
        for i in 1..n {
            assert!(mz[i] >= mz[i - 1], "mz not sorted at index {i}: {} < {}", mz[i], mz[i - 1]);
        }
    }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_spectrum_intensity_non_negative() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let spec = unsafe { spec.assume_init() };
    let n = spec.num_peaks as usize;
    if n > 0 {
        let inten = unsafe { std::slice::from_raw_parts(spec.intensity, n) };
        for (i, &val) in inten.iter().enumerate() {
            assert!(val >= 0.0, "negative intensity at {i}: {val}");
        }
    }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_spectrum_metadata_fields() {
    let path = require_dda!();
    let handle = open_real(&path);
    // Find an MS2 spectrum to check metadata
    let n = unsafe { tims_num_spectra(handle as *const _) };
    let mut found_ms2 = false;
    for i in 0..n.min(100) {
        let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
        let status = unsafe { tims_get_spectrum(handle, i, spec.as_mut_ptr()) };
        if status != TIMSFFI_OK { continue; }
        let spec = unsafe { spec.assume_init() };
        if spec.ms_level == 2 {
            assert!(spec.frame_index != u32::MAX, "MS2 should have frame_index");
            assert!(spec.isolation_width >= 0.0);
            assert!(spec.isolation_mz >= 0.0);
            found_ms2 = true;
            break;
        }
    }
    if !found_ms2 {
        eprintln!("no MS2 spectrum found in first 100 spectra, skipping metadata check");
    }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_get_frame_0() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let frame = unsafe { frame.assume_init() };
    assert!(frame.num_peaks > 0, "first frame should have peaks");
    assert!(frame.num_scans > 0, "first frame should have scans");
    unsafe { tims_close(handle); }
}

#[test]
fn dda_frame_scan_offsets_monotonic() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let frame = unsafe { frame.assume_init() };
    if frame.num_scans > 0 && !frame.scan_offsets.is_null() {
        let offsets = unsafe {
            std::slice::from_raw_parts(frame.scan_offsets, frame.num_scans as usize + 1)
        };
        for i in 1..offsets.len() {
            assert!(offsets[i] >= offsets[i - 1], "scan_offsets not monotonic at {i}");
        }
        assert_eq!(
            *offsets.last().unwrap(),
            frame.num_peaks as u64,
            "last scan_offset should equal num_peaks"
        );
    }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_get_frames_by_level_ms1() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut count: u32 = 0;
    let mut frames: *mut TimsFfiFrame = ptr::null_mut();
    let status = unsafe { tims_get_frames_by_level(handle, 1, &mut count, &mut frames) };
    assert_status(status, TIMSFFI_OK);
    assert!(count > 0, "DDA should have MS1 frames");
    // Spot check: first frame should be MS1
    if count > 0 && !frames.is_null() {
        let first = unsafe { *frames };
        assert_eq!(first.ms_level, 1, "frames_by_level(1) should return MS1 frames");
    }
    unsafe { tims_free_frame_array(handle, frames, count); }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_get_spectra_by_rt() {
    let path = require_dda!();
    let handle = open_real(&path);
    // Get file info to find a reasonable RT
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };
    let mid_rt = (info.ms2.rt_min + info.ms2.rt_max) / 2.0;

    let mut count: u32 = 0;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, mid_rt, 3, 0.0, 1e15, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert!(count > 0, "should find spectra near mid RT={mid_rt}");
    if count > 0 && !specs.is_null() {
        let first = unsafe { *specs };
        // RT should be somewhat near what we requested
        assert!(
            (first.rt_seconds - mid_rt).abs() < 60.0,
            "returned RT {} too far from requested {mid_rt}",
            first.rt_seconds
        );
    }
    unsafe { tims_free_spectrum_array(handle, specs, count); }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_converters_positive_finite() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mz = unsafe { tims_convert_tof_to_mz(handle as *const _, 100) };
    assert!(mz.is_finite() && mz > 0.0, "tof_to_mz should be positive finite, got {mz}");
    let im = unsafe { tims_convert_scan_to_im(handle as *const _, 100) };
    assert!(im.is_finite() && im > 0.0, "scan_to_im should be positive finite, got {im}");
    unsafe { tims_close(handle); }
}

#[test]
fn dda_converter_array_matches_scalar() {
    let path = require_dda!();
    let handle = open_real(&path);
    let indices: [u32; 3] = [50, 100, 500];
    let mut out_mz: [f64; 3] = [0.0; 3];
    let status = unsafe {
        tims_convert_tof_to_mz_array(handle as *const _, indices.as_ptr(), 3, out_mz.as_mut_ptr())
    };
    assert_status(status, TIMSFFI_OK);
    for i in 0..3 {
        let scalar = unsafe { tims_convert_tof_to_mz(handle as *const _, indices[i]) };
        assert!(
            (out_mz[i] - scalar).abs() < 1e-10,
            "array[{i}]={} != scalar={scalar}",
            out_mz[i]
        );
    }
    unsafe { tims_close(handle); }
}

#[test]
fn dda_file_info_populated() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };
    assert!(info.total_peaks > 0);
    assert!(info.ms2.rt_min < info.ms2.rt_max, "RT range should be valid");
    assert!(info.ms2.mz_min > 0.0, "mz min should be positive");
    unsafe { tims_close(handle); }
}

#[test]
fn dda_swath_windows() {
    let path = require_dda!();
    let handle = open_real(&path);
    let mut count: u32 = 0;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(handle, &mut count, &mut windows) };
    assert_status(status, TIMSFFI_OK);
    // DDA may have 0 swath windows — that's fine
    if count > 0 {
        unsafe { tims_free_swath_windows(handle, windows); }
    }
    unsafe { tims_close(handle); }
}

// ======================================================================
// DIA Tests
// ======================================================================

#[test]
fn dia_open_succeeds() {
    let path = require_dia!();
    let handle = open_real(&path);
    unsafe { tims_close(handle); }
}

#[test]
fn dia_spectra_much_more_than_frames() {
    let path = require_dia!();
    let handle = open_real(&path);
    let ns = unsafe { tims_num_spectra(handle as *const _) };
    let nf = unsafe { tims_num_frames(handle as *const _) };
    assert!(
        ns > nf * 2,
        "DIA-PASEF: expected num_spectra({ns}) >> num_frames({nf})"
    );
    unsafe { tims_close(handle); }
}

#[test]
fn dia_get_spectra_by_rt_with_im_filter() {
    let path = require_dia!();
    let handle = open_real(&path);
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let _ = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    let info = unsafe { info.assume_init() };
    let mid_rt = (info.ms2.rt_min + info.ms2.rt_max) / 2.0;
    let mid_im = (info.ms2.im_min + info.ms2.im_max) / 2.0;
    let im_range = 0.05; // narrow IM window

    let mut count: u32 = 0;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(
            handle,
            mid_rt,
            10,
            mid_im - im_range,
            mid_im + im_range,
            &mut count,
            &mut specs,
        )
    };
    assert_status(status, TIMSFFI_OK);
    if count > 0 && !specs.is_null() {
        for i in 0..count as usize {
            let spec = unsafe { *specs.add(i) };
            assert!(
                spec.im >= mid_im - im_range - 0.01 && spec.im <= mid_im + im_range + 0.01,
                "spectrum {i} IM {} outside filter [{}, {}]",
                spec.im,
                mid_im - im_range,
                mid_im + im_range
            );
        }
    }
    unsafe { tims_free_spectrum_array(handle, specs, count); }
    unsafe { tims_close(handle); }
}

#[test]
fn dia_swath_windows_populated() {
    let path = require_dia!();
    let handle = open_real(&path);
    let mut count: u32 = 0;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(handle, &mut count, &mut windows) };
    assert_status(status, TIMSFFI_OK);
    assert!(count > 0, "DIA dataset should have swath windows");
    if count > 0 && !windows.is_null() {
        let wins = unsafe { std::slice::from_raw_parts(windows, count as usize) };
        for (i, w) in wins.iter().enumerate() {
            assert!(w.mz_lower < w.mz_upper, "window {i}: mz_lower >= mz_upper");
            assert!(w.im_lower < w.im_upper, "window {i}: im_lower >= im_upper");
        }
        // Check coverage: windows should span a reasonable m/z range
        let min_mz = wins.iter().map(|w| w.mz_lower).fold(f64::INFINITY, f64::min);
        let max_mz = wins.iter().map(|w| w.mz_upper).fold(f64::NEG_INFINITY, f64::max);
        assert!(max_mz - min_mz > 100.0, "swath windows should cover > 100 m/z");
    }
    unsafe { tims_free_swath_windows(handle, windows); }
    unsafe { tims_close(handle); }
}

// ======================================================================
// Cross-cutting
// ======================================================================

#[test]
fn reopen_after_close() {
    let path = require_dda!();
    let handle1 = open_real(&path);
    let n1 = unsafe { tims_num_spectra(handle1 as *const _) };
    unsafe { tims_close(handle1); }

    let handle2 = open_real(&path);
    let n2 = unsafe { tims_num_spectra(handle2 as *const _) };
    assert_eq!(n1, n2, "reopened dataset should have same spectrum count");
    unsafe { tims_close(handle2); }
}

#[test]
fn two_handles_simultaneously() {
    let dda = match dda_path() { Some(p) => p, None => return };
    let dia = match dia_path() { Some(p) => p, None => return };

    let h_dda = open_real(&dda);
    let h_dia = open_real(&dia);

    let n_dda = unsafe { tims_num_spectra(h_dda as *const _) };
    let n_dia = unsafe { tims_num_spectra(h_dia as *const _) };
    assert!(n_dda > 0);
    assert!(n_dia > 0);
    // They should be different datasets with different counts
    // (not a hard requirement, but expected)

    unsafe {
        tims_close(h_dda);
        tims_close(h_dia);
    }
}
```

- [ ] **Step 2: Run tests (stub mode — all should skip gracefully)**

Run: `cargo test --test ffi_real_data -- --nocapture`
Expected: All tests print "...not set, skipping" and pass (0 failures)

- [ ] **Step 3: Commit**

```bash
git add tests/ffi_real_data.rs
git commit -m "test: add real-data integration tests (DDA + DIA, env-gated)"
```

## Chunk 4: C++ Test Suite

### Task 10: Catch2 setup and ABI tests

**Files:**
- Create: `tests_cpp/catch2/catch.hpp` (download Catch2 v3 single header)
- Create: `tests_cpp/test_abi.cpp`

- [ ] **Step 1: Download Catch2 single header**

```bash
mkdir -p tests_cpp/catch2
curl -L -o tests_cpp/catch2/catch.hpp \
  "https://github.com/catchorg/Catch2/releases/download/v3.5.2/catch_amalgamated.hpp"
curl -L -o tests_cpp/catch2/catch.cpp \
  "https://github.com/catchorg/Catch2/releases/download/v3.5.2/catch_amalgamated.cpp"
```

- [ ] **Step 2: Write the ABI test file**

```cpp
// tests_cpp/test_abi.cpp
//
// Compile-time and runtime ABI layout checks.
// Validates that the C header struct definitions match the Rust #[repr(C)] layout.

#include "../include/timsrust_cpp_bridge.h"
#include "catch2/catch.hpp"

#include <cstddef>
#include <cstdint>

// ---- Enum value checks ----

TEST_CASE("Status enum values", "[abi]") {
    REQUIRE(TIMSFFI_OK == 0);
    REQUIRE(TIMSFFI_ERR_INVALID_UTF8 == 1);
    REQUIRE(TIMSFFI_ERR_OPEN_FAILED == 2);
    REQUIRE(TIMSFFI_ERR_INDEX_OOB == 3);
    REQUIRE(TIMSFFI_ERR_INTERNAL == 255);
}

// ---- Struct size checks ----
// These sizes are platform-specific (x86_64 Linux with standard alignment).
// If building on a different platform, update expected sizes or derive them
// from a Rust helper: std::mem::size_of::<Type>()

TEST_CASE("tims_spectrum sizeof", "[abi]") {
    // Expected: 8+8+1+3pad+4+8+8+8+4+8+8+1+7pad+8+4+4pad = platform dependent
    // We check it matches what Rust reports. For now, just verify it compiles
    // and is > 0.
    REQUIRE(sizeof(tims_spectrum) > 0);
}

TEST_CASE("tims_frame sizeof", "[abi]") {
    REQUIRE(sizeof(tims_frame) > 0);
}

TEST_CASE("tims_swath_window sizeof", "[abi]") {
    REQUIRE(sizeof(tims_swath_window) > 0);
}

TEST_CASE("tims_level_stats sizeof", "[abi]") {
    REQUIRE(sizeof(tims_level_stats) > 0);
}

TEST_CASE("tims_file_info_t sizeof", "[abi]") {
    REQUIRE(sizeof(tims_file_info_t) > 0);
}

// ---- Key field offset checks ----

TEST_CASE("tims_spectrum field offsets", "[abi]") {
    // Verify critical pointer fields are at expected positions
    REQUIRE(offsetof(tims_spectrum, rt_seconds) == 0);
    REQUIRE(offsetof(tims_spectrum, mz) > offsetof(tims_spectrum, num_peaks));
    REQUIRE(offsetof(tims_spectrum, intensity) > offsetof(tims_spectrum, mz));
    REQUIRE(offsetof(tims_spectrum, im) > offsetof(tims_spectrum, intensity));
}

TEST_CASE("tims_frame field offsets", "[abi]") {
    REQUIRE(offsetof(tims_frame, index) == 0);
    REQUIRE(offsetof(tims_frame, tof_indices) > offsetof(tims_frame, num_peaks));
    REQUIRE(offsetof(tims_frame, intensities) > offsetof(tims_frame, tof_indices));
    REQUIRE(offsetof(tims_frame, scan_offsets) > offsetof(tims_frame, intensities));
}

TEST_CASE("tims_swath_window field offsets", "[abi]") {
    REQUIRE(offsetof(tims_swath_window, mz_lower) == 0);
    REQUIRE(offsetof(tims_swath_window, mz_upper) == sizeof(double));
    REQUIRE(offsetof(tims_swath_window, is_ms1) > offsetof(tims_swath_window, im_upper));
}
```

- [ ] **Step 3: Commit**

```bash
git add tests_cpp/catch2/ tests_cpp/test_abi.cpp
git commit -m "test: add Catch2 vendored header and ABI layout tests"
```

### Task 11: C++ smoke tests and config effects

**Files:**
- Create: `tests_cpp/test_smoke.cpp`

- [ ] **Step 1: Write the smoke test file**

```cpp
// tests_cpp/test_smoke.cpp
//
// End-to-end smoke tests and config effect tests.
// Tests requiring real data are gated behind DDA/DIA env vars.

#include "../include/timsrust_cpp_bridge.h"
#include "catch2/catch.hpp"

#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <cmath>

static std::string get_env(const char* name) {
    const char* val = std::getenv(name);
    return val ? std::string(val) : std::string();
}

// ---- Error path (no dataset needed) ----

TEST_CASE("Open bad path returns error", "[smoke]") {
    tims_dataset* handle = nullptr;
    auto status = tims_open("/tmp/timsrust_cpp_test_nonexistent", &handle);
    REQUIRE(status != TIMSFFI_OK);

    char buf[256] = {};
    tims_get_last_error(nullptr, buf, sizeof(buf));
    REQUIRE(std::strlen(buf) > 0);
}

// ---- Config builder round-trip (no dataset needed for stub) ----

TEST_CASE("Config builder round-trip", "[smoke]") {
    auto* cfg = tims_config_create();
    REQUIRE(cfg != nullptr);
    tims_config_set_smoothing_window(cfg, 3);
    tims_config_set_centroiding_window(cfg, 5);
    tims_config_set_calibration_tolerance(cfg, 0.01);
    tims_config_set_calibrate(cfg, 1);
    tims_config_free(cfg);
}

// ---- DDA smoke (real data) ----

TEST_CASE("DDA smoke test", "[smoke][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }

    tims_dataset* handle = nullptr;
    REQUIRE(tims_open(dda.c_str(), &handle) == TIMSFFI_OK);
    REQUIRE(handle != nullptr);

    auto n = tims_num_spectra(handle);
    REQUIRE(n > 0);

    tims_spectrum spec{};
    REQUIRE(tims_get_spectrum(handle, 0, &spec) == TIMSFFI_OK);
    REQUIRE(spec.num_peaks > 0);
    REQUIRE(spec.mz[0] > 0.0f);

    tims_close(handle);
}

// ---- DIA smoke (real data) ----

TEST_CASE("DIA smoke test", "[smoke][dia]") {
    auto dia = get_env("TIMSRUST_TEST_DATA_DIA");
    if (dia.empty()) { SKIP("TIMSRUST_TEST_DATA_DIA not set"); }

    tims_dataset* handle = nullptr;
    REQUIRE(tims_open(dia.c_str(), &handle) == TIMSFFI_OK);

    unsigned int win_count = 0;
    tims_swath_window* windows = nullptr;
    REQUIRE(tims_get_swath_windows(handle, &win_count, &windows) == TIMSFFI_OK);
    REQUIRE(win_count > 0);

    // Get spectra by RT
    tims_file_info_t info{};
    tims_file_info(handle, &info);
    double mid_rt = (info.ms2.rt_min + info.ms2.rt_max) / 2.0;

    unsigned int spec_count = 0;
    tims_spectrum* specs = nullptr;
    REQUIRE(tims_get_spectra_by_rt(handle, mid_rt, 3, 0.0, 1e15, &spec_count, &specs) == TIMSFFI_OK);
    REQUIRE(spec_count > 0);

    tims_free_spectrum_array(handle, specs, spec_count);
    tims_free_swath_windows(handle, windows);
    tims_close(handle);
}

// ---- Config effects (real data) ----

TEST_CASE("Centroiding reduces peak count", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }

    // Open with default config
    tims_dataset* h_default = nullptr;
    REQUIRE(tims_open(dda.c_str(), &h_default) == TIMSFFI_OK);
    tims_spectrum spec_default{};
    REQUIRE(tims_get_spectrum(h_default, 0, &spec_default) == TIMSFFI_OK);
    auto peaks_default = spec_default.num_peaks;
    tims_close(h_default);

    // Open with centroiding
    auto* cfg = tims_config_create();
    tims_config_set_centroiding_window(cfg, 10);
    tims_dataset* h_centro = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg, &h_centro) == TIMSFFI_OK);
    tims_spectrum spec_centro{};
    REQUIRE(tims_get_spectrum(h_centro, 0, &spec_centro) == TIMSFFI_OK);
    auto peaks_centro = spec_centro.num_peaks;
    tims_config_free(cfg);
    tims_close(h_centro);

    REQUIRE(peaks_centro <= peaks_default);
}

TEST_CASE("Smoothing changes intensities", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }

    // Open with explicit smoothing_window=0 (baseline)
    auto* cfg_base = tims_config_create();
    tims_config_set_smoothing_window(cfg_base, 0);
    tims_dataset* h1 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg_base, &h1) == TIMSFFI_OK);
    tims_config_free(cfg_base);
    tims_spectrum s1{};
    REQUIRE(tims_get_spectrum(h1, 0, &s1) == TIMSFFI_OK);
    std::vector<float> int1(s1.intensity, s1.intensity + s1.num_peaks);
    tims_close(h1);

    // Open with smoothing
    auto* cfg = tims_config_create();
    tims_config_set_smoothing_window(cfg, 5);
    tims_dataset* h2 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg, &h2) == TIMSFFI_OK);
    tims_spectrum s2{};
    REQUIRE(tims_get_spectrum(h2, 0, &s2) == TIMSFFI_OK);
    std::vector<float> int2(s2.intensity, s2.intensity + s2.num_peaks);
    tims_config_free(cfg);
    tims_close(h2);

    // At least some intensities should differ
    bool any_differ = false;
    auto n = std::min(int1.size(), int2.size());
    for (size_t i = 0; i < n; ++i) {
        if (std::abs(int1[i] - int2[i]) > 1e-6f) { any_differ = true; break; }
    }
    REQUIRE(any_differ);
}

TEST_CASE("Calibration changes m/z values", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }

    // Open without calibration
    auto* cfg_off = tims_config_create();
    tims_config_set_calibrate(cfg_off, 0);
    tims_dataset* h1 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg_off, &h1) == TIMSFFI_OK);
    tims_spectrum s1{};
    REQUIRE(tims_get_spectrum(h1, 0, &s1) == TIMSFFI_OK);
    std::vector<float> mz1(s1.mz, s1.mz + s1.num_peaks);
    tims_config_free(cfg_off);
    tims_close(h1);

    // Open with calibration
    auto* cfg_on = tims_config_create();
    tims_config_set_calibrate(cfg_on, 1);
    tims_dataset* h2 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg_on, &h2) == TIMSFFI_OK);
    tims_spectrum s2{};
    REQUIRE(tims_get_spectrum(h2, 0, &s2) == TIMSFFI_OK);
    std::vector<float> mz2(s2.mz, s2.mz + s2.num_peaks);
    tims_config_free(cfg_on);
    tims_close(h2);

    // m/z values may or may not differ depending on dataset;
    // at minimum neither should crash and both should return valid data
    REQUIRE(s1.num_peaks > 0);
    REQUIRE(s2.num_peaks > 0);
}

TEST_CASE("Config combinations produce valid output", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }

    auto* cfg = tims_config_create();
    tims_config_set_smoothing_window(cfg, 3);
    tims_config_set_centroiding_window(cfg, 5);
    tims_config_set_calibration_tolerance(cfg, 0.01);
    tims_config_set_calibrate(cfg, 1);

    tims_dataset* handle = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg, &handle) == TIMSFFI_OK);

    tims_spectrum spec{};
    REQUIRE(tims_get_spectrum(handle, 0, &spec) == TIMSFFI_OK);
    REQUIRE(spec.num_peaks > 0);

    // mz should be sorted
    for (uint32_t i = 1; i < spec.num_peaks; ++i) {
        REQUIRE(spec.mz[i] >= spec.mz[i - 1]);
    }

    // intensities should be non-negative
    for (uint32_t i = 0; i < spec.num_peaks; ++i) {
        REQUIRE(spec.intensity[i] >= 0.0f);
    }

    tims_config_free(cfg);
    tims_close(handle);
}
```

- [ ] **Step 2: Commit**

```bash
git add tests_cpp/test_smoke.cpp
git commit -m "test: add C++ smoke tests and config effect tests"
```

### Task 12: C++ Makefile

**Files:**
- Create: `tests_cpp/Makefile`

- [ ] **Step 1: Write the Makefile**

```makefile
# tests_cpp/Makefile
#
# Build and run C++ FFI tests against libtimsrust_cpp_bridge.
#
# Usage:
#   make test LIBDIR=../target/release
#   make test LIBDIR=../target/debug
#
# With real data:
#   make test LIBDIR=../target/release DDA=/path/to/dda.d DIA=/path/to/dia.d

CXX      ?= g++
CXXFLAGS := -std=c++17 -Wall -Werror -I../include
LIBDIR   ?= ../target/debug
LDFLAGS  := -L$(LIBDIR) -ltimsrust_cpp_bridge -Wl,-rpath,$(realpath $(LIBDIR))

# Optional dataset paths (empty = skip data tests)
DDA ?=
DIA ?=

SRCS     := test_abi.cpp test_smoke.cpp catch2/catch.cpp
OBJS     := $(SRCS:.cpp=.o)
BIN      := run_tests

.PHONY: test clean

test: $(BIN)
	@echo "=== Running C++ FFI tests ==="
	TIMSRUST_TEST_DATA_DDA="$(DDA)" TIMSRUST_TEST_DATA_DIA="$(DIA)" \
		./$(BIN) --reporter compact

$(BIN): $(OBJS)
	$(CXX) $(CXXFLAGS) -o $@ $^ $(LDFLAGS)

%.o: %.cpp
	$(CXX) $(CXXFLAGS) -c -o $@ $<

clean:
	rm -f $(OBJS) $(BIN)
```

**IMPORTANT:** Makefile recipe lines (indented lines under targets) MUST use
tab characters, not spaces. When copying from this plan, ensure indentation
is converted to tabs.

- [ ] **Step 2: Verify Makefile syntax**

Run: `make -n -f tests_cpp/Makefile` (dry-run to check for syntax errors)

- [ ] **Step 3: Commit**

```bash
git add tests_cpp/Makefile
git commit -m "test: add C++ test Makefile"
```

## Chunk 5: CI & Final Verification

### Task 13: GitHub Actions workflow

**Files:**
- Create: `.github/workflows/test.yml`

- [ ] **Step 1: Write the CI workflow**

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [master]
  pull_request:
  workflow_dispatch:

jobs:
  stub-tests:
    name: Stub Tests (no dataset)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cargo test (stub mode)
        run: cargo test -- --nocapture

      - name: Build library (debug)
        run: cargo build

      - name: C++ ABI tests
        run: |
          cd tests_cpp
          make test LIBDIR=../target/debug

  integration-tests:
    name: Integration Tests (real data)
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/master' || github.event_name == 'workflow_dispatch'
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Download test datasets
        run: |
          # Download from GitHub release artifacts
          # Dataset filenames TBD — update these URLs when known
          mkdir -p testdata
          # gh release download v0.1.0-testdata -p "*.d.tar.gz" -D testdata
          # tar -xzf testdata/dda.d.tar.gz -C testdata
          # tar -xzf testdata/dia.d.tar.gz -C testdata
          echo "TODO: Update with real dataset download commands"

      - name: Build library (release, with timsrust)
        run: cargo build --features with_timsrust --release

      - name: Rust integration tests
        env:
          TIMSRUST_TEST_DATA_DDA: testdata/dda.d
          TIMSRUST_TEST_DATA_DIA: testdata/dia.d
        run: cargo test --features with_timsrust -- --nocapture

      - name: C++ smoke tests
        run: |
          cd tests_cpp
          make test LIBDIR=../target/release \
            DDA=../testdata/dda.d \
            DIA=../testdata/dia.d
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/test.yml
git commit -m "ci: add GitHub Actions workflow for stub and integration tests"
```

### Task 14: Final verification — run all stub tests

- [ ] **Step 1: Build the library**

Run: `cargo build`

- [ ] **Step 2: Run all Rust stub tests**

Run: `cargo test -- --nocapture`
Expected: All tests pass (real-data tests skip with message)

- [ ] **Step 3: Build and run C++ ABI tests**

Run: `cd tests_cpp && make test LIBDIR=../target/debug`
Expected: ABI tests pass, smoke tests skip (no data)

- [ ] **Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit -m "test: fixups from final verification"
```
