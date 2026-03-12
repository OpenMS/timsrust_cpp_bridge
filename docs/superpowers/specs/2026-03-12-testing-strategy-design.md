# Testing Strategy Design — timsrust_cpp_bridge

## Overview

Contract-centric testing strategy for the timsrust_cpp_bridge FFI library. Rust integration tests provide the bulk of coverage (~80%) for FFI safety, memory ownership, and error handling. A focused C++ Catch2 suite (~20%) validates ABI layout, header correctness, and end-to-end flows including config effects (centroiding, smoothing, calibration).

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Primary goal | FFI safety + data correctness | Layered: stub mode for contract, real data for correctness |
| Test language split | 80% Rust / 20% C++ | Rust catches memory/safety bugs; C++ catches ABI drift |
| Approach | Contract-centric (Approach B) | Tests organized by FFI contract area, not source files |
| C++ framework | Catch2 (header-only, vendored) | Single header drop-in, no build system complexity |
| C++ runner | Standalone (Makefile) | Two-step: `cargo build` then `make test` |
| Rust runner | Standard `cargo test` | No extra tooling needed |
| Test data | External datasets from GitHub release artifacts | Env vars `TIMSRUST_TEST_DATA_DDA` / `TIMSRUST_TEST_DATA_DIA`; dataset filenames TBD |

## File Layout

```
tests/
├── ffi_lifecycle.rs        # open, close, open_with_config, config builder
├── ffi_error_handling.rs    # error codes, tims_get_last_error, global vs per-handle
├── ffi_spectrum.rs          # tims_get_spectrum, tims_get_spectra_by_rt, free
├── ffi_frame.rs             # tims_get_frame, tims_get_frames_by_level, free
├── ffi_query.rs             # num_spectra, num_frames, swath windows, file_info
├── ffi_converters.rs        # tof_to_mz, scan_to_im, array variants
├── ffi_real_data.rs         # real dataset tests (DDA + DIA), skipped if env var unset
├── common/
│   └── mod.rs               # shared helpers: open_stub(), assert_status(), env var check

tests_cpp/
├── catch2/
│   └── catch.hpp            # Catch2 single-header (vendored)
├── test_abi.cpp             # struct offsetof/sizeof checks, header compilation
├── test_smoke.cpp           # open->query->close with real data, config effects
└── Makefile                 # builds and runs C++ tests against libtimsrust_cpp_bridge
```

## Rust Test Coverage — Stub Mode

All tests run against the stub build (no `with_timsrust` feature). No datasets required.

### Lifecycle (`ffi_lifecycle.rs`)

- open with null path -> returns `INVALID_UTF8`, handle is null
- open with nonexistent path -> returns `OPEN_FAILED`
- open with valid stub -> returns `OK`, handle is non-null
- close null handle -> no crash (null-safe)
- close valid handle -> no crash, no double-free
- config builder lifecycle -> create config, set all 4 fields, open_with_config, close config, close handle
- config create returns non-null
- config free null -> no crash

### Error Handling (`ffi_error_handling.rs`)

- get_last_error with null handle -> reads global error
- get_last_error with valid handle -> reads per-handle error
- get_last_error with null buffer -> returns `INTERNAL`
- get_last_error with zero-length buffer -> returns `INTERNAL`
- get_last_error truncation -> small buffer gets truncated message, null-terminated
- error after successful operation -> error cleared / empty
- error message content -> after open failure, message contains meaningful text

### Single Spectrum (`ffi_spectrum.rs`)

- get_spectrum index 0 from stub -> returns `OK`, num_peaks is 0
- get_spectrum out of bounds -> returns `INDEX_OOB`
- get_spectrum with null handle -> returns `INTERNAL`
- get_spectrum with null out param -> returns `INTERNAL`
- buffer invalidation — call get_spectrum, save pointers, call again, document that old pointers are invalid

### Batch Spectrum (`ffi_spectrum.rs`)

- get_spectra_by_rt stub -> returns OK, count=0, null pointer
- free_spectrum_array with null -> no crash
- free_spectrum_array with count=0 -> no crash

### Single Frame (`ffi_frame.rs`)

- get_frame index 0 from stub -> returns `OK`, num_peaks is 0
- get_frame out of bounds -> returns `INDEX_OOB`
- get_frame with null handle -> returns `INTERNAL`
- get_frame with null out param -> returns `INTERNAL`
- buffer invalidation — mirror of spectrum test

### Batch Frame (`ffi_frame.rs`)

- get_frames_by_level stub -> returns OK, count=0, null pointer
- free_frame_array with null -> no crash
- free_frame_array with count=0 -> no crash

### Query & Metadata (`ffi_query.rs`)

- num_spectra on stub -> returns 0
- num_frames on stub -> returns 0
- get_swath_windows stub -> returns OK, count=0
- free_swath_windows null -> no crash
- file_info stub -> returns OK, all fields zero

### Converters (`ffi_converters.rs`)

- tof_to_mz with null handle -> returns NaN
- scan_to_im with null handle -> returns NaN
- tof_to_mz stub -> returns identity value
- tof_to_mz_array with null params -> returns `INTERNAL`
- tof_to_mz_array stub -> output matches identity
- scan_to_im_array — mirror of tof tests

## Rust Test Coverage — Real Data Mode

Tests in `ffi_real_data.rs`. Gated behind env vars; skip gracefully if unset. Require `--features with_timsrust`.

### DDA Dataset Tests

- open succeeds -> status OK, handle non-null
- num_spectra > 0
- num_frames > 0, and num_frames <= num_spectra
- get_spectrum(0) -> OK, num_peaks > 0, mz/intensity non-null, rt > 0, ms_level is 1 or 2
- spectrum mz values sorted (monotonically non-decreasing)
- spectrum intensity values non-negative
- get_frame(0) -> OK, num_peaks > 0, num_scans > 0, scan_offsets length = num_scans + 1
- frame scan_offsets monotonic, last offset == num_peaks
- get_spectra_by_rt -> pick RT from middle, request n_spec=3, verify count > 0, returned RT near requested
- converters produce positive finite values
- converter array matches scalar (call array variant, compare to loop of scalar calls)
- file_info -> total_peaks > 0, rt min < max, mz range positive
- swath_windows -> for DDA, count may be 0

### DIA Dataset Tests

- open succeeds
- num_spectra >> num_frames (DIA-PASEF expansion)
- get_spectra_by_rt with IM filter -> narrow drift range, verify IM within range
- swath_windows -> count > 0, mz_lower < mz_upper, im_lower < im_upper
- swath window coverage -> windows collectively cover a reasonable m/z range

### Cross-cutting

- open then close then open -> no stale global state
- two handles simultaneously -> open DDA and DIA, query both, close both

## C++ Test Suite (Catch2)

### ABI Layout (`test_abi.cpp`)

- `sizeof` checks via `static_assert` for all C-repr structs
- `offsetof` checks for key fields (mz/intensity pointers, tof_indices, scan_offsets)
- Enum value checks (TIMSFFI_OK == 0, TIMSFFI_ERR_INDEX_OOB == 3, etc.)
- Header compiles with `-Wall -Werror`

### Smoke Test (`test_smoke.cpp`)

Gated behind dataset env vars, same as Rust real-data tests.

- Config builder round-trip -> create, set fields, open_with_config, close
- DDA smoke -> open, num_spectra, get_spectrum(0), verify peaks > 0 and mz[0] > 0, close
- DIA smoke -> open, get_swath_windows (count > 0), get_spectra_by_rt, free, close
- Error path -> open bad path, verify status != OK, get_last_error returns non-empty message

### Config Effects (`test_smoke.cpp`)

Validates that config builder options produce observable effects on output. All require real data.

- Centroiding reduces peak count -> open default vs open with centroiding_window, compare num_peaks (centroided <= original)
- Smoothing changes intensities -> open with smoothing_window=0 vs N, verify intensity arrays differ
- Calibration changes m/z values -> open with calibrate on vs off, verify mz arrays differ (or at minimum no crash)
- Config combinations -> centroiding + smoothing + calibration all set, verify OK with reasonable data (num_peaks > 0, mz sorted, intensities non-negative)

### Makefile

```makefile
# Build and run:
#   make test LIBDIR=../target/release
# With real data:
#   make test LIBDIR=../target/release DDA=/path/to.d DIA=/path/to.d
```

## CI Strategy (GitHub Actions)

### Job 1: Stub Tests (every push/PR)

- `cargo test` — all Rust FFI tests against stub build
- `cd tests_cpp && make test LIBDIR=../target/debug` — ABI layout checks only (smoke tests skipped)
- Fast, no data dependencies

### Job 2: Integration Tests (push to master, manual trigger)

- Downloads DDA and DIA datasets from GitHub release artifacts
- `cargo build --features with_timsrust --release`
- Runs Rust real-data tests with env vars
- Runs C++ smoke tests with dataset paths
- Validates correctness against real data

### Feature Gate Handling

- `cargo test` (no features) -> stub-mode tests
- `cargo test --features with_timsrust` -> same tests with real reader; stub-specific assertions gated behind `#[cfg(not(feature = "with_timsrust"))]`
- Real-data tests require `with_timsrust` AND env vars — skip if either missing

## Test Helpers (`tests/common/mod.rs`)

```rust
// Opens a dataset in stub mode (creates temp dir as fake .d path)
pub fn open_stub() -> *mut tims_dataset { ... }

// Asserts FFI status code
pub fn assert_status(status: TimsFfiStatus, expected: TimsFfiStatus) { ... }

// Returns DDA/DIA path from env, or None (caller returns early)
pub fn dda_path() -> Option<String> { ... }
pub fn dia_path() -> Option<String> { ... }
```
