# Sage API Parity Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose all timsrust functionality that Sage uses through the C FFI bridge — frame-level access, converters, configurable reader construction, and extended spectrum metadata.

**Architecture:** Four independent feature areas added incrementally: (1) extend existing `TimsFfiSpectrum` with missing fields, (2) add raw frame-level access via `FrameReader`, (3) expose TOF→m/z and scan→IM converters, (4) add opaque config builder for `SpectrumReaderConfig`. Each area follows the existing pattern of `types.rs` structs → `dataset.rs` logic → `lib.rs` FFI exports → C header updates. Both `with_timsrust` and stub builds are maintained.

**Tech Stack:** Rust (FFI via `extern "C"`, `#[repr(C)]`), timsrust 0.4.2, libc for malloc/free, C17 header

**Spec:** `docs/superpowers/specs/2026-03-11-sage-api-parity-design.md`

---

## Chunk 1: Extended TimsFfiSpectrum

### Task 1: Add new fields to TimsFfiSpectrum

**Files:**
- Modify: `src/types.rs:4-14`

- [ ] **Step 1: Add new fields to the struct**

In `src/types.rs`, extend `TimsFfiSpectrum` by appending after `im`:

```rust
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
    // New fields for Sage parity
    pub index: c_uint,               // Spectrum.index from SpectrumReader
    pub isolation_width: c_double,   // isolation window width (0.0 if N/A)
    pub isolation_mz: c_double,      // isolation window center m/z (0.0 if N/A)
    pub charge: c_uchar,             // precursor charge (0 = unknown)
    pub precursor_intensity: c_double, // precursor intensity (f64::NAN = unknown)
    pub frame_index: c_uint,         // precursor frame index (u32::MAX = N/A, i.e. MS1)
}
```

- [ ] **Step 2: Verify stub build compiles**

Run: `cargo check`
Expected: Compilation errors in `dataset.rs` and `lib.rs` where `TimsFfiSpectrum` is constructed without the new fields. This confirms the type change propagated.

### Task 2: Populate new fields in get_spectrum (dataset.rs)

**Files:**
- Modify: `src/dataset.rs:161-223` (the `get_spectrum` method)

- [ ] **Step 1: Update the with_timsrust branch of get_spectrum**

In `src/dataset.rs`, replace the `#[cfg(feature = "with_timsrust")]` block inside `get_spectrum` to populate new fields:

```rust
        #[cfg(feature = "with_timsrust")]
        {
            let spec = self.reader.get(index as usize)
                .map_err(|_| TimsFfiStatus::IndexOutOfBounds)?;

            let n = spec.len();
            self.mz_buf.clear();
            self.mz_buf.reserve(n);
            for &v in spec.mz_values.iter() {
                self.mz_buf.push(v as f32);
            }
            self.int_buf.clear();
            self.int_buf.reserve(n);
            for &v in spec.intensities.iter() {
                self.int_buf.push(v as f32);
            }

            out.num_peaks = n as u32;
            out.mz = if n == 0 { ptr::null() } else { self.mz_buf.as_ptr() };
            out.intensity = if n == 0 { ptr::null() } else { self.int_buf.as_ptr() };
            out.index = spec.index as u32;
            out.isolation_width = spec.isolation_width;
            out.isolation_mz = spec.isolation_mz;

            if let Some(prec) = spec.precursor {
                out.rt_seconds = prec.rt;
                out.precursor_mz = prec.mz;
                out.im = prec.im;
                out.ms_level = 2;
                out.charge = prec.charge.map(|c| c as u8).unwrap_or(0);
                out.precursor_intensity = prec.intensity.unwrap_or(f64::NAN);
                out.frame_index = prec.frame_index as u32;
            } else {
                out.rt_seconds = 0.0;
                out.precursor_mz = 0.0;
                out.im = 0.0;
                out.ms_level = 1;
                out.charge = 0;
                out.precursor_intensity = f64::NAN;
                out.frame_index = u32::MAX;
            }

            return Ok(());
        }
```

- [ ] **Step 2: Update the stub branch of get_spectrum**

In the `#[cfg(not(feature = "with_timsrust"))]` block, add the new fields with sentinel values:

```rust
        #[cfg(not(feature = "with_timsrust"))]
        {
            out.rt_seconds = 0.0;
            out.precursor_mz = 0.0;
            out.ms_level = 0;
            out.num_peaks = 0;
            out.mz = ptr::null();
            out.intensity = ptr::null();
            out.im = 0.0;
            out.index = 0;
            out.isolation_width = 0.0;
            out.isolation_mz = 0.0;
            out.charge = 0;
            out.precursor_intensity = f64::NAN;
            out.frame_index = u32::MAX;
            Ok(())
        }
```

- [ ] **Step 3: Verify stub build compiles**

Run: `cargo check`
Expected: May still fail due to `TimsFfiSpectrum` construction in `lib.rs` (`tims_get_spectra_by_rt`). That's expected — fixed in the next task.

### Task 3: Populate new fields in tims_get_spectra_by_rt (lib.rs)

**Files:**
- Modify: `src/lib.rs:315-323` (the `TimsFfiSpectrum` literal in `tims_get_spectra_by_rt`)

- [ ] **Step 1: Update the TimsFfiSpectrum construction in tims_get_spectra_by_rt**

Replace the `let out_spec = TimsFfiSpectrum { ... }` block (around line 315) with:

```rust
            let out_spec = TimsFfiSpectrum {
                rt_seconds: spec.precursor.map(|p| p.rt).unwrap_or(0.0),
                precursor_mz: spec.precursor.map(|p| p.mz).unwrap_or(0.0),
                ms_level: if spec.precursor.is_some() { 2 } else { 1 },
                num_peaks: n as u32,
                mz: if mz_ptr.is_null() { std::ptr::null() } else { mz_ptr as *const f32 },
                intensity: if int_ptr.is_null() { std::ptr::null() } else { int_ptr as *const f32 },
                im: spec.precursor.map(|p| p.im).unwrap_or(0.0),
                index: spec.index as u32,
                isolation_width: spec.isolation_width,
                isolation_mz: spec.isolation_mz,
                charge: spec.precursor.and_then(|p| p.charge).map(|c| c as u8).unwrap_or(0),
                precursor_intensity: spec.precursor.and_then(|p| p.intensity).unwrap_or(f64::NAN),
                frame_index: spec.precursor.map(|p| p.frame_index as u32).unwrap_or(u32::MAX),
            };
```

- [ ] **Step 2: Verify stub build compiles cleanly**

Run: `cargo check`
Expected: PASS — all `TimsFfiSpectrum` construction sites now include new fields.

- [ ] **Step 3: Also verify with timsrust feature (if available)**

Run: `cargo check --features with_timsrust`
Expected: PASS (confirms timsrust field names like `spec.isolation_width`, `spec.isolation_mz`, `prec.charge`, `prec.intensity`, `prec.frame_index` are correct). If this fails due to missing timsrust dependency in the environment, defer to CI.

- [ ] **Step 4: Update C header tims_spectrum struct**

In `include/timsrust_cpp_bridge.h`, replace the existing `tims_spectrum` typedef to match the Rust struct:

```c
typedef struct {
    double   rt_seconds;
    double   precursor_mz;
    uint8_t  ms_level;
    uint32_t num_peaks;
    const float* mz;
    const float* intensity;
    double   im;
    /* Sage-parity fields */
    uint32_t index;               /* spectrum index from SpectrumReader */
    double   isolation_width;     /* isolation window width (0.0 if N/A) */
    double   isolation_mz;        /* isolation window center m/z (0.0 if N/A) */
    uint8_t  charge;              /* precursor charge (0 = unknown) */
    double   precursor_intensity; /* precursor intensity (NaN = unknown) */
    uint32_t frame_index;         /* precursor frame index (UINT32_MAX for MS1) */
} tims_spectrum;
```

- [ ] **Step 5: Commit**

```bash
git add src/types.rs src/dataset.rs src/lib.rs include/timsrust_cpp_bridge.h
git commit -m "feat: extend TimsFfiSpectrum with index, isolation, charge, precursor_intensity, frame_index"
```

---

## Chunk 2: Frame-Level Access

### Task 4: Add TimsFfiFrame type

**Files:**
- Modify: `src/types.rs`

- [ ] **Step 1: Add TimsFfiFrame struct**

Append to `src/types.rs` (before the closing of the file):

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimsFfiFrame {
    pub index: c_uint,
    pub rt_seconds: c_double,
    pub ms_level: c_uchar,        // 1=MS1, 2=MS2, 0=Unknown
    pub num_scans: c_uint,
    pub num_peaks: c_uint,         // total peaks (length of tof_indices & intensities)
    pub tof_indices: *const u32,   // raw TOF indices, flat array
    pub intensities: *const u32,   // raw intensities, flat array
    pub scan_offsets: *const u64,  // per-scan offsets (length: num_scans + 1)
}
```

- [ ] **Step 2: Verify stub build compiles**

Run: `cargo check`
Expected: PASS — the new type is defined but not yet used.

### Task 5: Add FrameReader and converters to TimsDataset

**Files:**
- Modify: `src/dataset.rs`

This is the largest task. It adds `FrameReader`, converters, frame buffers, and methods for frame access.

- [ ] **Step 1: Add timsrust imports for FrameReader, MetadataReader, converters**

At the top of `src/dataset.rs`, after the existing `#[cfg(feature = "with_timsrust")]` imports, add:

```rust
#[cfg(feature = "with_timsrust")]
use timsrust::readers::FrameReader;
#[cfg(feature = "with_timsrust")]
use timsrust::converters::{Tof2MzConverter, Scan2ImConverter};
```

Also add the new type import to the `use crate::types` line:

```rust
use crate::types::{TimsFfiSpectrum, TimsFfiSwathWindow, TimsFfiFrame, TimsFfiStatus};
```

- [ ] **Step 2: Add stub FrameReader and converter types**

After the existing stub `SpectrumReader` block, add stubs for `FrameReader` and converters:

```rust
#[cfg(not(feature = "with_timsrust"))]
struct FrameReader {
    n: usize,
}

#[cfg(not(feature = "with_timsrust"))]
impl FrameReader {
    fn new(_path: &str) -> Result<Self, ()> {
        Ok(FrameReader { n: 0 })
    }
    fn len(&self) -> usize {
        self.n
    }
}

#[cfg(not(feature = "with_timsrust"))]
struct Tof2MzConverter;

#[cfg(not(feature = "with_timsrust"))]
struct Scan2ImConverter;

#[cfg(not(feature = "with_timsrust"))]
impl Tof2MzConverter {
    fn convert(&self, value: f64) -> f64 { value }
}

#[cfg(not(feature = "with_timsrust"))]
impl Scan2ImConverter {
    fn convert(&self, value: f64) -> f64 { value }
}
```

- [ ] **Step 3: Extend TimsDataset struct with new fields**

Replace the `TimsDataset` struct definition:

```rust
pub struct TimsDataset {
    pub(crate) reader: SpectrumReader,
    // Reusable buffers for mz/intensity (spectrum access)
    mz_buf: Vec<f32>,
    int_buf: Vec<f32>,
    // Reusable buffers for frame access (independent of spectrum buffers)
    frame_tof_buf: Vec<u32>,
    frame_int_buf: Vec<u32>,
    frame_scan_offset_buf: Vec<u64>,
    // FrameReader for raw frame-level access (pub(crate) for lib.rs batch access)
    pub(crate) frame_reader: FrameReader,
    // Converters: TOF index → m/z, scan index → ion mobility
    pub(crate) mz_converter: Tof2MzConverter,
    pub(crate) im_converter: Scan2ImConverter,
    // Optional cached swath windows computed at open time when available
    swath_windows: Option<Vec<TimsFfiSwathWindow>>,
    // Optional last error string for this handle
    pub(crate) last_error: Option<String>,
    // RT index: sorted (rt_seconds, spectrum_index) pairs for fast lookup
    rt_index: Vec<(f64, usize)>,
}
```

Note: `num_frames` field removed — replaced by `frame_reader.len()`.

- [ ] **Step 4: Update the `open` method — with_timsrust branch**

In the `#[cfg(feature = "with_timsrust")]` section of `open()`, replace the `(swath_windows, num_frames)` block to also construct `frame_reader` and extract converters. The FrameReader and MetadataReader are already being created temporarily — now we keep them:

```rust
        #[cfg(feature = "with_timsrust")]
        let (swath_windows, frame_reader, mz_converter, im_converter) = {
            use timsrust::readers::QuadrupoleSettingsReader;

            let fr = FrameReader::new(path_str)
                .map_err(|_| TimsFfiStatus::OpenFailed)?;

            let metadata = MetadataReader::new(path_str)
                .map_err(|_| TimsFfiStatus::OpenFailed)?;
            let mz_conv = metadata.mz_converter;
            let im_conv = metadata.im_converter;

            let mut out: Vec<TimsFfiSwathWindow> = Vec::new();
            if let Ok(quads) = QuadrupoleSettingsReader::new(path_str) {
                for quad in quads.iter() {
                    for i in 0..quad.len() {
                        let center = quad.isolation_mz[i];
                        let width = quad.isolation_width[i];
                        let mz_lower = center - width / 2.0;
                        let mz_upper = center + width / 2.0;
                        let (im_lower, im_upper) = if i < quad.scan_starts.len() && i < quad.scan_ends.len() {
                            let start = quad.scan_starts[i] as f64;
                            let end = quad.scan_ends[i] as f64;
                            let im_start = im_conv.convert(start);
                            let im_end = im_conv.convert(end);
                            (im_start.min(im_end), im_start.max(im_end))
                        } else {
                            (metadata.lower_im, metadata.upper_im)
                        };
                        out.push(TimsFfiSwathWindow {
                            mz_lower,
                            mz_upper,
                            mz_center: center,
                            im_lower,
                            im_upper,
                            is_ms1: 0,
                        });
                    }
                }
            }
            let sw = if out.is_empty() { None } else { Some(out) };
            (sw, fr, mz_conv, im_conv)
        };
```

- [ ] **Step 5: Update the `open` method — stub branch**

Replace the stub branch:

```rust
        #[cfg(not(feature = "with_timsrust"))]
        let (swath_windows, frame_reader, mz_converter, im_converter) = {
            let fr = FrameReader::new("").map_err(|_| TimsFfiStatus::OpenFailed)?;
            (None, fr, Tof2MzConverter, Scan2ImConverter)
        };
```

- [ ] **Step 6: Update the struct construction in `open`**

Replace `Ok(TimsDataset { ... })` at the end of `open()`:

```rust
        Ok(TimsDataset {
            reader,
            mz_buf: Vec::new(),
            int_buf: Vec::new(),
            frame_tof_buf: Vec::new(),
            frame_int_buf: Vec::new(),
            frame_scan_offset_buf: Vec::new(),
            frame_reader,
            mz_converter,
            im_converter,
            swath_windows,
            last_error: None,
            rt_index,
        })
```

- [ ] **Step 7: Update `num_frames()` to use frame_reader**

Replace the `num_frames` method:

```rust
    pub fn num_frames(&self) -> u32 {
        self.frame_reader.len() as u32
    }
```

Remove the `num_frames` field comment if present.

- [ ] **Step 8: Add get_frame method**

Add after the `get_swath_windows` method:

```rust
    /// Get a single frame by index. Buffers are handle-owned,
    /// valid until the next call to get_frame on this handle.
    pub fn get_frame(&mut self, index: u32, out: &mut TimsFfiFrame)
        -> Result<(), TimsFfiStatus>
    {
        let len = self.frame_reader.len() as u32;
        if index >= len {
            return Err(TimsFfiStatus::IndexOutOfBounds);
        }

        #[cfg(feature = "with_timsrust")]
        {
            let frame = self.frame_reader.get(index as usize)
                .map_err(|_| TimsFfiStatus::IndexOutOfBounds)?;

            self.frame_tof_buf.clear();
            self.frame_tof_buf.extend_from_slice(&frame.tof_indices);

            self.frame_int_buf.clear();
            self.frame_int_buf.extend_from_slice(&frame.intensities);

            self.frame_scan_offset_buf.clear();
            self.frame_scan_offset_buf.extend(frame.scan_offsets.iter().map(|&s| s as u64));

            let num_scans = if frame.scan_offsets.is_empty() {
                0
            } else {
                (frame.scan_offsets.len() - 1) as u32
            };

            out.index = frame.index as u32;
            out.rt_seconds = frame.rt_in_seconds;
            out.ms_level = match frame.ms_level {
                timsrust::MSLevel::MS1 => 1,
                timsrust::MSLevel::MS2 => 2,
                _ => 0,
            };
            out.num_scans = num_scans;
            out.num_peaks = frame.tof_indices.len() as u32;
            out.tof_indices = if out.num_peaks == 0 { ptr::null() } else { self.frame_tof_buf.as_ptr() };
            out.intensities = if out.num_peaks == 0 { ptr::null() } else { self.frame_int_buf.as_ptr() };
            out.scan_offsets = if num_scans == 0 { ptr::null() } else { self.frame_scan_offset_buf.as_ptr() };

            return Ok(());
        }

        #[cfg(not(feature = "with_timsrust"))]
        {
            out.index = index;
            out.rt_seconds = 0.0;
            out.ms_level = 0;
            out.num_scans = 0;
            out.num_peaks = 0;
            out.tof_indices = ptr::null();
            out.intensities = ptr::null();
            out.scan_offsets = ptr::null();
            Ok(())
        }
    }
```

Note: The `match frame.ms_level` may need adjustment depending on the exact `MSLevel` enum variants in timsrust 0.4.2. Use a wildcard `_` for any unknown variants.

- [ ] **Step 9: Verify stub build compiles**

Run: `cargo check`
Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add src/types.rs src/dataset.rs
git commit -m "feat: add TimsFfiFrame type, FrameReader, and converters to TimsDataset"
```

### Task 6: Add frame FFI exports in lib.rs

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Add import for TimsFfiFrame**

Update the `use crate::types` import at the top of `lib.rs`:

```rust
use crate::types::{TimsFfiSpectrum, TimsFfiStatus, TimsFfiFileInfo, TimsFfiLevelStats, TimsFfiFrame};
```

- [ ] **Step 2: Add tims_get_frame FFI function**

Append after the `tims_file_info` function:

```rust
#[no_mangle]
pub extern "C" fn tims_get_frame(
    handle: *mut tims_dataset,
    index: c_uint,
    out_frame: *mut TimsFfiFrame,
) -> TimsFfiStatus {
    if handle.is_null() || out_frame.is_null() {
        return TimsFfiStatus::Internal;
    }
    let ds = unsafe { &mut (*handle).inner };
    let out = unsafe { &mut *out_frame };
    match ds.get_frame(index, out) {
        Ok(()) => TimsFfiStatus::Ok,
        Err(e) => e,
    }
}
```

- [ ] **Step 3: Add tims_get_frames_by_level FFI function**

```rust
#[no_mangle]
pub extern "C" fn tims_get_frames_by_level(
    handle: *mut tims_dataset,
    ms_level: u8,
    out_frames: *mut *mut TimsFfiFrame,
    out_count: *mut c_uint,
) -> TimsFfiStatus {
    if handle.is_null() || out_frames.is_null() || out_count.is_null() {
        return TimsFfiStatus::Internal;
    }

    // Invalid ms_level → empty result
    if ms_level != 1 && ms_level != 2 {
        unsafe { *out_count = 0; *out_frames = std::ptr::null_mut(); }
        return TimsFfiStatus::Ok;
    }

    #[cfg(not(feature = "with_timsrust"))]
    {
        unsafe { *out_count = 0; *out_frames = std::ptr::null_mut(); }
        return TimsFfiStatus::Ok;
    }

    #[cfg(feature = "with_timsrust")]
    {
        let ds = unsafe { &mut (*handle).inner };

        let frames_result = if ms_level == 1 {
            ds.frame_reader.get_all_ms1()
        } else {
            ds.frame_reader.get_all_ms2()
        };

        // Collect frames, failing on any read error
        let frames: Vec<_> = match frames_result.into_iter().collect::<Result<Vec<_>, _>>() {
            Ok(v) => v,
            Err(_) => {
                ds.last_error = Some("failed to read one or more frames".to_string());
                unsafe { *out_count = 0; *out_frames = std::ptr::null_mut(); }
                return TimsFfiStatus::Internal;
            }
        };
        let n = frames.len();

        if n == 0 {
            unsafe { *out_count = 0; *out_frames = std::ptr::null_mut(); }
            return TimsFfiStatus::Ok;
        }

        // Allocate output array
        let arr_ptr = unsafe { malloc(n * mem::size_of::<TimsFfiFrame>()) } as *mut TimsFfiFrame;
        if arr_ptr.is_null() {
            return TimsFfiStatus::Internal;
        }

        for (i, frame) in frames.iter().enumerate() {
            let num_scans = if frame.scan_offsets.is_empty() {
                0u32
            } else {
                (frame.scan_offsets.len() - 1) as u32
            };
            let n_peaks = frame.tof_indices.len();

            // Allocate per-frame arrays via malloc
            let tof_ptr = if n_peaks == 0 { std::ptr::null_mut() } else {
                unsafe { malloc(n_peaks * mem::size_of::<u32>()) } as *mut u32
            };
            let int_ptr = if n_peaks == 0 { std::ptr::null_mut() } else {
                unsafe { malloc(n_peaks * mem::size_of::<u32>()) } as *mut u32
            };
            let scan_len = frame.scan_offsets.len();
            let scan_ptr = if scan_len == 0 { std::ptr::null_mut() } else {
                unsafe { malloc(scan_len * mem::size_of::<u64>()) } as *mut u64
            };

            // Check allocations
            if (n_peaks > 0 && (tof_ptr.is_null() || int_ptr.is_null()))
                || (scan_len > 0 && scan_ptr.is_null())
            {
                // Free already-allocated frames
                for j in 0..i {
                    unsafe {
                        let old = arr_ptr.add(j).read();
                        if !old.tof_indices.is_null() { free(old.tof_indices as *mut libc::c_void); }
                        if !old.intensities.is_null() { free(old.intensities as *mut libc::c_void); }
                        if !old.scan_offsets.is_null() { free(old.scan_offsets as *mut libc::c_void); }
                    }
                }
                if !tof_ptr.is_null() { unsafe { free(tof_ptr as *mut libc::c_void); } }
                if !int_ptr.is_null() { unsafe { free(int_ptr as *mut libc::c_void); } }
                if !scan_ptr.is_null() { unsafe { free(scan_ptr as *mut libc::c_void); } }
                unsafe { free(arr_ptr as *mut libc::c_void); }
                return TimsFfiStatus::Internal;
            }

            // Copy data
            if n_peaks > 0 {
                unsafe {
                    std::ptr::copy_nonoverlapping(frame.tof_indices.as_ptr(), tof_ptr, n_peaks);
                    std::ptr::copy_nonoverlapping(frame.intensities.as_ptr(), int_ptr, n_peaks);
                }
            }
            if scan_len > 0 {
                for (k, &offset) in frame.scan_offsets.iter().enumerate() {
                    unsafe { *scan_ptr.add(k) = offset as u64; }
                }
            }

            let ms_lvl: u8 = match frame.ms_level {
                timsrust::MSLevel::MS1 => 1,
                timsrust::MSLevel::MS2 => 2,
                _ => 0,
            };

            let out_frame = TimsFfiFrame {
                index: frame.index as u32,
                rt_seconds: frame.rt_in_seconds,
                ms_level: ms_lvl,
                num_scans,
                num_peaks: n_peaks as u32,
                tof_indices: if tof_ptr.is_null() { std::ptr::null() } else { tof_ptr },
                intensities: if int_ptr.is_null() { std::ptr::null() } else { int_ptr },
                scan_offsets: if scan_ptr.is_null() { std::ptr::null() } else { scan_ptr },
            };
            unsafe { arr_ptr.add(i).write(out_frame); }
        }

        unsafe { *out_count = n as c_uint; *out_frames = arr_ptr; }
        TimsFfiStatus::Ok
    }
}
```

- [ ] **Step 4: Add tims_free_frame_array FFI function**

```rust
#[no_mangle]
pub extern "C" fn tims_free_frame_array(
    _handle: *mut tims_dataset,
    frames: *mut TimsFfiFrame,
    count: c_uint,
) {
    if frames.is_null() { return; }
    let n = count as usize;
    for i in 0..n {
        unsafe {
            let f = frames.add(i).read();
            if !f.tof_indices.is_null() { free(f.tof_indices as *mut libc::c_void); }
            if !f.intensities.is_null() { free(f.intensities as *mut libc::c_void); }
            if !f.scan_offsets.is_null() { free(f.scan_offsets as *mut libc::c_void); }
        }
    }
    unsafe { free(frames as *mut libc::c_void); }
}
```

- [ ] **Step 5: Verify stub build compiles**

Run: `cargo check`
Expected: PASS

- [ ] **Step 6: Also verify with timsrust feature (if available)**

Run: `cargo check --features with_timsrust`
Expected: PASS (confirms `FrameReader::get`, `get_all_ms1`/`get_all_ms2`, `Frame` field names). If this fails due to missing timsrust dependency, defer to CI.

- [ ] **Step 7: Update C header with frame types and functions**

In `include/timsrust_cpp_bridge.h`, add after the `tims_swath_window` typedef:

```c
typedef struct {
    uint32_t  index;          /* frame index */
    double    rt_seconds;     /* retention time in seconds */
    uint8_t   ms_level;       /* 1=MS1, 2=MS2, 0=Unknown */
    uint32_t  num_scans;      /* number of scans */
    uint32_t  num_peaks;      /* total peaks (length of tof_indices & intensities) */
    const uint32_t *tof_indices;    /* raw TOF indices, flat array */
    const uint32_t *intensities;    /* raw intensities, flat array */
    const uint64_t *scan_offsets;   /* per-scan offsets (length: num_scans + 1) */
} tims_frame;
```

And add the function declarations before the `#ifdef __cplusplus` closing:

```c
/* -------------------------------------------------------------------------
 * Frame-level access (raw TOF indices, not converted to m/z)
 * ------------------------------------------------------------------------- */

/* Get a single frame by index. Buffers are handle-owned, valid until the
 * next call to tims_get_frame on the same handle. Frame and spectrum
 * buffers are independent. */
timsffi_status tims_get_frame(tims_dataset* handle, uint32_t index, tims_frame* out);

/* Get all frames at a given MS level (1=MS1, 2=MS2).
 * Returns malloc'd array; free with tims_free_frame_array.
 * Invalid ms_level returns empty array with TIMSFFI_OK. */
timsffi_status tims_get_frames_by_level(tims_dataset* handle, uint8_t ms_level,
                                        tims_frame** out_frames, uint32_t* out_count);

/* Free frames allocated by tims_get_frames_by_level. Frees per-frame
 * tof_indices, intensities, scan_offsets arrays, then the array itself. */
void tims_free_frame_array(tims_dataset* handle, tims_frame* frames, uint32_t count);
```

- [ ] **Step 8: Commit**

```bash
git add src/lib.rs include/timsrust_cpp_bridge.h
git commit -m "feat: add tims_get_frame, tims_get_frames_by_level, tims_free_frame_array FFI exports"
```

---

## Chunk 3: Converters and Config

### Task 7: Add converter FFI exports

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Add single-value converter functions**

Append to `src/lib.rs`:

```rust
#[no_mangle]
pub extern "C" fn tims_convert_tof_to_mz(
    handle: *const tims_dataset,
    tof_index: c_uint,
) -> c_double {
    if handle.is_null() { return f64::NAN; }
    let ds = unsafe { &(*handle).inner };
    ds.mz_converter.convert(tof_index as f64)
}

#[no_mangle]
pub extern "C" fn tims_convert_scan_to_im(
    handle: *const tims_dataset,
    scan_index: c_uint,
) -> c_double {
    if handle.is_null() { return f64::NAN; }
    let ds = unsafe { &(*handle).inner };
    ds.im_converter.convert(scan_index as f64)
}
```

Note: For the `with_timsrust` build, `ConvertableDomain::convert()` is a trait method already imported in `dataset.rs`. Since `lib.rs` accesses the converters via `ds.mz_converter` and `ds.im_converter`, we need the trait in scope. Add at the top of `lib.rs`:

```rust
#[cfg(feature = "with_timsrust")]
use timsrust::converters::ConvertableDomain;
```

- [ ] **Step 2: Add batch converter functions**

```rust
#[no_mangle]
pub extern "C" fn tims_convert_tof_to_mz_array(
    handle: *const tims_dataset,
    tof_indices: *const u32,
    count: c_uint,
    out_mz: *mut c_double,
) -> TimsFfiStatus {
    if handle.is_null() || tof_indices.is_null() || out_mz.is_null() {
        return TimsFfiStatus::Internal;
    }
    let ds = unsafe { &(*handle).inner };
    let n = count as usize;
    for i in 0..n {
        let idx = unsafe { *tof_indices.add(i) };
        unsafe { *out_mz.add(i) = ds.mz_converter.convert(idx as f64); }
    }
    TimsFfiStatus::Ok
}

#[no_mangle]
pub extern "C" fn tims_convert_scan_to_im_array(
    handle: *const tims_dataset,
    scan_indices: *const u32,
    count: c_uint,
    out_im: *mut c_double,
) -> TimsFfiStatus {
    if handle.is_null() || scan_indices.is_null() || out_im.is_null() {
        return TimsFfiStatus::Internal;
    }
    let ds = unsafe { &(*handle).inner };
    let n = count as usize;
    for i in 0..n {
        let idx = unsafe { *scan_indices.add(i) };
        unsafe { *out_im.add(i) = ds.im_converter.convert(idx as f64); }
    }
    TimsFfiStatus::Ok
}
```

- [ ] **Step 3: Verify stub build compiles**

Run: `cargo check`
Expected: PASS — the stub `Tof2MzConverter` and `Scan2ImConverter` have a `convert` method.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs
git commit -m "feat: add tims_convert_tof_to_mz and tims_convert_scan_to_im FFI exports"
```

### Task 8: Add config module

**Files:**
- Create: `src/config.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create src/config.rs**

```rust
// src/config.rs
//
// Opaque config builder wrapping timsrust's SpectrumReaderConfig.
// Used by tims_open_with_config() to customize reader construction.

#[cfg(feature = "with_timsrust")]
use timsrust::readers::SpectrumReaderConfig;

/// FFI-facing config wrapper. When with_timsrust is enabled, wraps the
/// real SpectrumReaderConfig. Otherwise a dummy struct for API compat.
pub struct TimsFfiConfig {
    #[cfg(feature = "with_timsrust")]
    pub(crate) inner: SpectrumReaderConfig,

    #[cfg(not(feature = "with_timsrust"))]
    _dummy: (),
}

impl TimsFfiConfig {
    pub fn new() -> Self {
        TimsFfiConfig {
            #[cfg(feature = "with_timsrust")]
            inner: SpectrumReaderConfig::default(),

            #[cfg(not(feature = "with_timsrust"))]
            _dummy: (),
        }
    }

    #[cfg(feature = "with_timsrust")]
    pub fn set_smoothing_window(&mut self, window: u32) {
        self.inner.spectrum_processing_params.smoothing_window = window;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_smoothing_window(&mut self, _window: u32) {}

    #[cfg(feature = "with_timsrust")]
    pub fn set_centroiding_window(&mut self, window: u32) {
        self.inner.spectrum_processing_params.centroiding_window = window;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_centroiding_window(&mut self, _window: u32) {}

    #[cfg(feature = "with_timsrust")]
    pub fn set_calibration_tolerance(&mut self, tolerance: f64) {
        self.inner.spectrum_processing_params.calibration_tolerance = tolerance;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_calibration_tolerance(&mut self, _tolerance: f64) {}

    #[cfg(feature = "with_timsrust")]
    pub fn set_calibrate(&mut self, enabled: bool) {
        self.inner.spectrum_processing_params.calibrate = enabled;
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn set_calibrate(&mut self, _enabled: bool) {}
}
```

Note: The exact field names on `SpectrumProcessingParams` (`smoothing_window`, `centroiding_window`, `calibration_tolerance`, `calibrate`) must be verified against timsrust 0.4.2 at build time. If the field names differ, adjust accordingly. The `FrameWindowSplittingConfiguration` setters are deferred (TBD in spec).

- [ ] **Step 2: Register config module in lib.rs**

Add `mod config;` at the top of `src/lib.rs` alongside `mod dataset;` and `mod types;`:

```rust
mod config;
mod dataset;
mod types;
```

- [ ] **Step 3: Add config FFI exports to lib.rs**

Add the `use` import and FFI functions:

```rust
use crate::config::TimsFfiConfig;
```

Then add the FFI functions:

```rust
/// Opaque config type for C callers.
#[repr(C)]
pub struct tims_config {
    inner: TimsFfiConfig,
}

#[no_mangle]
pub extern "C" fn tims_config_create() -> *mut tims_config {
    let cfg = Box::new(tims_config {
        inner: TimsFfiConfig::new(),
    });
    Box::into_raw(cfg)
}

#[no_mangle]
pub extern "C" fn tims_config_free(cfg: *mut tims_config) {
    if cfg.is_null() { return; }
    unsafe { drop(Box::from_raw(cfg)); }
}

#[no_mangle]
pub extern "C" fn tims_config_set_smoothing_window(cfg: *mut tims_config, window: c_uint) {
    if cfg.is_null() { return; }
    let c = unsafe { &mut (*cfg).inner };
    c.set_smoothing_window(window);
}

#[no_mangle]
pub extern "C" fn tims_config_set_centroiding_window(cfg: *mut tims_config, window: c_uint) {
    if cfg.is_null() { return; }
    let c = unsafe { &mut (*cfg).inner };
    c.set_centroiding_window(window);
}

#[no_mangle]
pub extern "C" fn tims_config_set_calibration_tolerance(cfg: *mut tims_config, tolerance: c_double) {
    if cfg.is_null() { return; }
    let c = unsafe { &mut (*cfg).inner };
    c.set_calibration_tolerance(tolerance);
}

#[no_mangle]
pub extern "C" fn tims_config_set_calibrate(cfg: *mut tims_config, enabled: u8) {
    if cfg.is_null() { return; }
    let c = unsafe { &mut (*cfg).inner };
    c.set_calibrate(enabled != 0);
}
```

- [ ] **Step 4: Add tims_open_with_config FFI function**

This function also needs a corresponding `open_with_config` method on `TimsDataset` in `dataset.rs`, or we can handle the builder pattern directly in `lib.rs`. Since the config affects only the `SpectrumReader` construction (via builder), add the logic in `lib.rs` (similar to `tims_open`) and add an `open_with_config` to `dataset.rs`.

First, add to `src/dataset.rs`:

```rust
    #[cfg(feature = "with_timsrust")]
    pub fn open_with_config(path: &CStr, config: &TimsFfiConfig) -> Result<Self, TimsFfiStatus> {
        let path_str = path.to_str().map_err(|_| TimsFfiStatus::InvalidUtf8)?;

        let reader = timsrust::readers::SpectrumReader::build()
            .with_path(path_str)
            .with_config(config.inner.clone())
            .finalize()
            .map_err(|_| TimsFfiStatus::OpenFailed)?;

        // Rest is same as open() — extract frame_reader, converters, swath windows, RT index
        // Factor out the common post-reader setup
        Self::finish_open(path_str, reader)
    }
```

This means we should refactor `open()` to share common setup. Add a helper:

In `dataset.rs`, under `#[cfg(feature = "with_timsrust")]`, add a private helper after the struct:

```rust
    #[cfg(feature = "with_timsrust")]
    fn finish_open(path_str: &str, reader: SpectrumReader) -> Result<Self, TimsFfiStatus> {
        use timsrust::readers::QuadrupoleSettingsReader;
        use timsrust::readers::PrecursorReader;

        let fr = FrameReader::new(path_str)
            .map_err(|_| TimsFfiStatus::OpenFailed)?;

        let metadata = MetadataReader::new(path_str)
            .map_err(|_| TimsFfiStatus::OpenFailed)?;
        let mz_conv = metadata.mz_converter;
        let im_conv = metadata.im_converter;

        let mut sw_out: Vec<TimsFfiSwathWindow> = Vec::new();
        if let Ok(quads) = QuadrupoleSettingsReader::new(path_str) {
            for quad in quads.iter() {
                for i in 0..quad.len() {
                    let center = quad.isolation_mz[i];
                    let width = quad.isolation_width[i];
                    let mz_lower = center - width / 2.0;
                    let mz_upper = center + width / 2.0;
                    let (im_lower, im_upper) = if i < quad.scan_starts.len() && i < quad.scan_ends.len() {
                        let start = quad.scan_starts[i] as f64;
                        let end = quad.scan_ends[i] as f64;
                        let im_start = im_conv.convert(start);
                        let im_end = im_conv.convert(end);
                        (im_start.min(im_end), im_start.max(im_end))
                    } else {
                        (metadata.lower_im, metadata.upper_im)
                    };
                    sw_out.push(TimsFfiSwathWindow {
                        mz_lower,
                        mz_upper,
                        mz_center: center,
                        im_lower,
                        im_upper,
                        is_ms1: 0,
                    });
                }
            }
        }
        let swath_windows = if sw_out.is_empty() { None } else { Some(sw_out) };

        // Build RT index
        let mut rt_index: Vec<(f64, usize)> = Vec::new();
        if let Ok(prec_reader) = PrecursorReader::build()
            .with_path(path_str)
            .finalize()
        {
            let n = prec_reader.len();
            rt_index.reserve(n);
            for i in 0..n {
                if let Some(p) = prec_reader.get(i) {
                    rt_index.push((p.rt, i));
                }
            }
            rt_index.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        Ok(TimsDataset {
            reader,
            mz_buf: Vec::new(),
            int_buf: Vec::new(),
            frame_tof_buf: Vec::new(),
            frame_int_buf: Vec::new(),
            frame_scan_offset_buf: Vec::new(),
            frame_reader: fr,
            mz_converter: mz_conv,
            im_converter: im_conv,
            swath_windows,
            last_error: None,
            rt_index,
        })
    }
```

Then simplify `open()` for the `with_timsrust` branch:

```rust
    pub fn open(path: &CStr) -> Result<Self, TimsFfiStatus> {
        let path_str = path.to_str().map_err(|_| TimsFfiStatus::InvalidUtf8)?;

        #[cfg(feature = "with_timsrust")]
        {
            let reader = SpectrumReader::new(path_str)
                .map_err(|_| TimsFfiStatus::OpenFailed)?;
            return Self::finish_open(path_str, reader);
        }

        #[cfg(not(feature = "with_timsrust"))]
        {
            let reader = SpectrumReader::new(path)
                .map_err(|_| TimsFfiStatus::OpenFailed)?;
            let frame_reader = FrameReader::new("").map_err(|_| TimsFfiStatus::OpenFailed)?;
            Ok(TimsDataset {
                reader,
                mz_buf: Vec::new(),
                int_buf: Vec::new(),
                frame_tof_buf: Vec::new(),
                frame_int_buf: Vec::new(),
                frame_scan_offset_buf: Vec::new(),
                frame_reader,
                mz_converter: Tof2MzConverter,
                im_converter: Scan2ImConverter,
                swath_windows: None,
                last_error: None,
                rt_index: Vec::new(),
            })
        }
    }

    #[cfg(feature = "with_timsrust")]
    pub fn open_with_config(path: &CStr, config: &crate::config::TimsFfiConfig) -> Result<Self, TimsFfiStatus> {
        let path_str = path.to_str().map_err(|_| TimsFfiStatus::InvalidUtf8)?;
        let reader = timsrust::readers::SpectrumReader::build()
            .with_path(path_str)
            .with_config(config.inner.clone())
            .finalize()
            .map_err(|_| TimsFfiStatus::OpenFailed)?;
        Self::finish_open(path_str, reader)
    }

    #[cfg(not(feature = "with_timsrust"))]
    pub fn open_with_config(path: &CStr, _config: &crate::config::TimsFfiConfig) -> Result<Self, TimsFfiStatus> {
        Self::open(path)
    }
```

- [ ] **Step 5: Add tims_open_with_config FFI function in lib.rs**

```rust
#[no_mangle]
pub extern "C" fn tims_open_with_config(
    path: *const c_char,
    cfg: *const tims_config,
    out_handle: *mut *mut tims_dataset,
) -> TimsFfiStatus {
    if path.is_null() || cfg.is_null() || out_handle.is_null() {
        return TimsFfiStatus::Internal;
    }
    let cstr = unsafe { CStr::from_ptr(path) };
    let path_str = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return TimsFfiStatus::InvalidUtf8,
    };
    if std::fs::metadata(path_str).is_err() {
        if let Ok(mut g) = LAST_ERROR.lock() { *g = Some(format!("path not found: {}", path_str)); }
        return TimsFfiStatus::OpenFailed;
    }

    let path_owned = path_str.to_string();

    let (tx, rx) = mpsc::channel();
    // Clone the inner SpectrumReaderConfig for the thread.
    // SpectrumReaderConfig derives Clone/Copy in timsrust 0.4.2.
    #[cfg(feature = "with_timsrust")]
    let config_inner = unsafe { (*cfg).inner.inner.clone() };

    let thr = thread::spawn(move || {
        let c = CString::new(path_owned).unwrap();
        let res = std::panic::catch_unwind(|| {
            TimsDataset::open_with_config(c.as_c_str(), &{
                let mut cfg = crate::config::TimsFfiConfig::new();
                #[cfg(feature = "with_timsrust")]
                { cfg.inner = config_inner; }
                cfg
            })
        });
        match res {
            Ok(Ok(inner)) => { let _ = tx.send(Ok(inner)); }
            Ok(Err(e)) => { let _ = tx.send(Err(format!("open error: {:?}", e))); }
            Err(p) => {
                let msg = if let Some(s) = p.downcast_ref::<&str>() { s.to_string() }
                    else if let Some(s) = p.downcast_ref::<String>() { s.clone() }
                    else { "panic during open".to_string() };
                let _ = tx.send(Err(format!("panic: {}", msg)));
            }
        }
    });

    let got = match thr.join() {
        Ok(_) => rx.recv(),
        Err(_) => Err(std::sync::mpsc::RecvError),
    };

    match got {
        Ok(Ok(inner)) => {
            let boxed = Box::new(tims_dataset { inner });
            unsafe { *out_handle = Box::into_raw(boxed); }
            if let Ok(mut g) = LAST_ERROR.lock() { *g = None; }
            TimsFfiStatus::Ok
        }
        Ok(Err(msg)) => {
            if let Ok(mut g) = LAST_ERROR.lock() { *g = Some(msg); }
            TimsFfiStatus::OpenFailed
        }
        Err(_) => {
            if let Ok(mut g) = LAST_ERROR.lock() { *g = Some("open join/recv failed".to_string()); }
            TimsFfiStatus::OpenFailed
        }
    }
}
```

- [ ] **Step 6: Verify stub build compiles**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Also verify with timsrust feature (if available)**

Run: `cargo check --features with_timsrust`
Expected: PASS (confirms `SpectrumReaderConfig`, `SpectrumProcessingParams` field names, `ConvertableDomain::convert`). If this fails due to missing timsrust dependency, defer to CI.

- [ ] **Step 8: Update C header with converter and config declarations**

In `include/timsrust_cpp_bridge.h`, add before the `#ifdef __cplusplus` closing:

```c
/* -------------------------------------------------------------------------
 * Index converters (TOF -> m/z, scan -> ion mobility)
 * ------------------------------------------------------------------------- */

/* Convert a single TOF index to m/z. Returns NaN if handle is NULL. */
double tims_convert_tof_to_mz(const tims_dataset* handle, uint32_t tof_index);

/* Convert a single scan index to ion mobility (1/K0). Returns NaN if handle is NULL. */
double tims_convert_scan_to_im(const tims_dataset* handle, uint32_t scan_index);

/* Batch convert TOF indices to m/z. Caller provides output buffer. */
timsffi_status tims_convert_tof_to_mz_array(const tims_dataset* handle,
                                             const uint32_t* tof_indices, uint32_t count,
                                             double* out_mz);

/* Batch convert scan indices to ion mobility. Caller provides output buffer. */
timsffi_status tims_convert_scan_to_im_array(const tims_dataset* handle,
                                              const uint32_t* scan_indices, uint32_t count,
                                              double* out_im);

/* -------------------------------------------------------------------------
 * Opaque configuration for SpectrumReader construction
 * ------------------------------------------------------------------------- */

typedef struct tims_config tims_config;

/* Create a new config with default values. Caller must free with tims_config_free. */
tims_config *tims_config_create(void);

/* Free a config created by tims_config_create. */
void tims_config_free(tims_config *cfg);

/* SpectrumProcessingParams setters */
void tims_config_set_smoothing_window(tims_config *cfg, uint32_t window);
void tims_config_set_centroiding_window(tims_config *cfg, uint32_t window);
void tims_config_set_calibration_tolerance(tims_config *cfg, double tolerance);
void tims_config_set_calibrate(tims_config *cfg, uint8_t enabled); /* 0=off, non-zero=on */

/* Open dataset with custom config. Existing tims_open uses defaults. */
timsffi_status tims_open_with_config(const char* path, const tims_config* cfg, tims_dataset** out);
```

- [ ] **Step 9: Commit**

```bash
git add src/config.rs src/dataset.rs src/lib.rs include/timsrust_cpp_bridge.h
git commit -m "feat: add config builder, tims_open_with_config, and converter FFI exports"
```

---

## Chunk 4: C++ Example

### Task 9: Update C++ example

**Files:**
- Modify: `examples/cpp_client.cpp`

- [ ] **Step 1: Add frame and converter demonstration**

Add a new section after the file info output (before `tims_close`). This demonstrates frame access, converters, and the new spectrum fields:

```cpp
    // ---- Frame-level access demo --------------------------------------------
    std::cout << "\n-- Frame-level access --\n";
    unsigned int total_frames = tims_num_frames(handle);
    if (total_frames > 0) {
        tims_frame frame{};
        timsffi_status fs = tims_get_frame(handle, 0, &frame);
        if (fs == TIMSFFI_OK) {
            std::cout << "Frame 0: index=" << frame.index
                      << "  rt=" << std::fixed << std::setprecision(2) << frame.rt_seconds << "s"
                      << "  ms_level=" << (int)frame.ms_level
                      << "  scans=" << frame.num_scans
                      << "  peaks=" << frame.num_peaks << "\n";
        }

        // Batch: get all MS1 frames
        tims_frame* ms1_frames = nullptr;
        unsigned int ms1_count = 0;
        auto t_ms1 = Clock::now();
        tims_get_frames_by_level(handle, 1, &ms1_frames, &ms1_count);
        double ms1_ms = elapsed_ms(t_ms1);
        std::cout << "MS1 frames: " << ms1_count
                  << "  (fetched in " << std::setprecision(1) << ms1_ms << " ms)\n";
        if (ms1_frames) tims_free_frame_array(handle, ms1_frames, ms1_count);
    }

    // ---- Converter demo -----------------------------------------------------
    std::cout << "\n-- Converters --\n";
    double mz_example = tims_convert_tof_to_mz(handle, 100000);
    double im_example = tims_convert_scan_to_im(handle, 500);
    std::cout << "TOF 100000 -> m/z " << std::setprecision(4) << mz_example << "\n";
    std::cout << "Scan 500   -> IM  " << std::setprecision(4) << im_example << "\n";

    // ---- Extended spectrum fields demo --------------------------------------
    std::cout << "\n-- Extended spectrum fields --\n";
    if (tims_num_spectra(handle) > 0) {
        tims_spectrum spec{};
        if (tims_get_spectrum(handle, 0, &spec) == TIMSFFI_OK) {
            std::cout << "Spectrum 0: index=" << spec.index
                      << "  ms_level=" << (int)spec.ms_level
                      << "  charge=" << (int)spec.charge
                      << "  isolation_width=" << std::setprecision(2) << spec.isolation_width
                      << "  isolation_mz=" << spec.isolation_mz
                      << "  frame_index=" << spec.frame_index
                      << "  precursor_intensity=";
            if (std::isnan(spec.precursor_intensity))
                std::cout << "N/A";
            else
                std::cout << std::setprecision(0) << spec.precursor_intensity;
            std::cout << "\n";
        }
    }
```

- [ ] **Step 2: Verify stub build compiles**

Run: `cargo check`
Expected: PASS (the C++ example is compiled separately, not by cargo)

- [ ] **Step 3: Commit**

```bash
git add examples/cpp_client.cpp
git commit -m "feat: update C++ example with frame, converter, and extended spectrum demos"
```

---

## Implementation Notes

### Build verification

Since there is no automated test suite, verification at each step uses:
- `cargo check` — fast type-check for the stub build (no `with_timsrust` feature)
- `cargo check --features with_timsrust` — type-check with real timsrust (requires the dependency to be available)
- `cargo build --release` — full stub build
- `cargo build --features with_timsrust --release` — full build with timsrust

### timsrust API verification

Some field names (`SpectrumProcessingParams.smoothing_window`, `Frame.rt_in_seconds`, `Spectrum.isolation_mz`, etc.) are based on the Sage analysis and timsrust 0.4.2 docs. If any field name doesn't match at compile time, check the actual timsrust source and adjust. Key types to verify:
- `timsrust::Frame` — fields: `tof_indices`, `intensities`, `scan_offsets`, `rt_in_seconds`, `index`, `ms_level`
- `timsrust::Spectrum` — fields: `isolation_width`, `isolation_mz`, `index`
- `timsrust::Precursor` — fields: `charge`, `intensity`, `frame_index`
- `timsrust::MSLevel` — variants: `MS1`, `MS2`, and possibly `Unknown`
- `timsrust::readers::SpectrumReaderConfig` — field: `spectrum_processing_params`
- `timsrust::SpectrumProcessingParams` — fields: `smoothing_window`, `centroiding_window`, `calibration_tolerance`, `calibrate`

### FrameReader methods

The plan uses `FrameReader::get(index)` for single-frame access and `FrameReader::get_all_ms1()` / `FrameReader::get_all_ms2()` for batch. Verify these methods exist in timsrust 0.4.2. If `get_all_ms1/ms2` don't exist, use `parallel_filter` with the appropriate predicate instead (requires rayon in scope).

### ConvertableDomain trait

The `convert()` method is from the `ConvertableDomain` trait. It must be in scope wherever `.convert()` is called. In `dataset.rs` it's already imported. In `lib.rs` it needs to be imported for the converter FFI functions if they call `.convert()` directly (which they do via `ds.mz_converter.convert()`).
