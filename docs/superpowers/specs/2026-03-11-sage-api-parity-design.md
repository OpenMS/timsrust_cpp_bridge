# Sage API Parity — Design Spec

Expose all timsrust functionality that [Sage](https://github.com/lazear/sage) uses through the timsrust_cpp_bridge FFI.

## Context

Sage (sage-cloudpath crate) uses timsrust for:
- **SpectrumReader** with configurable `SpectrumReaderConfig` (processing params, DIA frame splitting)
- **FrameReader** for raw MS1 frame-level access (tof_indices, intensities, scan_offsets)
- **MetadataReader** for `Tof2MzConverter` and `Scan2ImConverter` (raw index → physical value conversion)
- **Spectrum** fields not yet exposed: `isolation_width`, `index`, `precursor.charge`, `precursor.intensity`, `precursor.frame_index`

The current bridge exposes spectrum-level access only, with no frame-level access, no converters, and no configurable reader construction.

## Design

### 1. Extended `TimsFfiSpectrum`

Append new fields to the existing struct (no existing field offsets change):

```c
typedef struct tims_spectrum {
    // existing
    double   rt_seconds;
    double   precursor_mz;
    uint8_t  ms_level;
    uint32_t num_peaks;
    float   *mz;
    float   *intensity;
    double   im;
    // new
    uint32_t index;               // spectrum index from SpectrumReader (Spectrum.index)
    double   isolation_width;     // isolation window width (0.0 if N/A)
    double   isolation_mz;        // isolation window center m/z (0.0 if N/A)
    uint8_t  charge;              // precursor charge (0 = unknown)
    double   precursor_intensity; // precursor intensity (NaN = unknown)
    uint32_t frame_index;         // precursor's frame index (UINT32_MAX if N/A)
} tims_spectrum;
```

Sentinel values for optional fields: `0` for charge, `UINT32_MAX` for frame_index (emitted when the spectrum has no precursor, i.e. MS1), `NaN` for precursor_intensity, `0.0` for isolation_width/isolation_mz. Keeps the struct flat and C-friendly.

Notes:
- `precursor.charge` is `Option<usize>` in timsrust — the `usize → u8` cast is safe since charge values are always small (1–6 in practice).
- `precursor.intensity` is `Option<f64>`, preserved as `double` to avoid precision loss.
- `precursor.frame_index` is a plain `usize` (not optional) in timsrust — the `UINT32_MAX` sentinel applies only to MS1 spectra where no `Precursor` exists.

### 2. Frame-Level Access

#### New type: `TimsFfiFrame`

```c
typedef struct tims_frame {
    uint32_t  index;          // frame index
    double    rt_seconds;     // retention time
    uint8_t   ms_level;       // 1=MS1, 2=MS2, 0=Unknown
    uint32_t  num_scans;      // number of scans (derived as frame.scan_offsets.len() - 1)
    uint32_t  num_peaks;      // total peaks (length of tof_indices & intensities)
    uint32_t *tof_indices;    // raw TOF indices, flat array
    uint32_t *intensities;    // raw intensities, flat array
    uint64_t *scan_offsets;   // per-scan offsets into flat arrays (length: num_scans + 1)
} tims_frame;
```

Raw indices are preserved (not converted to m/z) so callers can perform efficient discrete-domain operations like binning/summing on TOF indices before converting.

Implementation notes:
- `scan_offsets` is `Vec<usize>` in timsrust. The bridge copies to `Vec<u64>` for a stable 64-bit ABI. 32-bit targets are not supported.
- `ms_level` maps from timsrust's `MSLevel` enum: `MS1 → 1`, `MS2 → 2`, `Unknown → 0`.

#### New functions

**Single-frame access (handle-owned buffers):**
```c
tims_status tims_get_frame(tims_dataset *ds, uint32_t index, tims_frame *out);
```
Buffers are owned by the dataset handle, valid until the next call to `tims_get_frame` on that handle. Frame and spectrum buffers are independent — calling `tims_get_spectrum` does not invalidate frame buffers and vice versa.

**Batch filtered access (caller-owned, malloc'd):**
```c
tims_status tims_get_frames_by_level(
    tims_dataset *ds,
    uint8_t ms_level,
    tims_frame **out_frames,
    uint32_t *out_count
);
void tims_free_frame_array(tims_dataset *ds, tims_frame *frames, uint32_t count);
```
Uses `FrameReader::get_all_ms1()` / `get_all_ms2()` on the Rust side (internally parallel). Invalid `ms_level` values (anything other than 1 or 2) return an empty array with `out_count = 0` and `Ok` status.

`tims_free_frame_array` frees per-frame `tof_indices`, `intensities`, and `scan_offsets` arrays, then the frame array itself.

### 3. Converters

Methods on the dataset handle. `MetadataReader::new()` is called at open time, and the returned `Metadata`'s converters (`mz_converter`, `im_converter`) are cached inside `TimsDataset`.

**Single-value conversion:**
```c
double tims_convert_tof_to_mz(tims_dataset *ds, uint32_t tof_index);
double tims_convert_scan_to_im(tims_dataset *ds, uint32_t scan_index);
```

**Batch conversion (caller-provided output buffer):**
```c
tims_status tims_convert_tof_to_mz_array(
    tims_dataset *ds,
    const uint32_t *tof_indices, uint32_t count,
    double *out_mz
);
tims_status tims_convert_scan_to_im_array(
    tims_dataset *ds,
    const uint32_t *scan_indices, uint32_t count,
    double *out_im
);
```

Batch versions take caller-provided output buffers (no malloc — caller knows the size). Single-value versions return the result directly (converter is always valid once dataset is open). Returns `NaN` if handle is NULL.

### 4. Configurable Reader Construction

**Opaque config builder:**
```c
typedef struct tims_config tims_config;

tims_config *tims_config_create(void);
void         tims_config_free(tims_config *cfg);

// SpectrumProcessingParams setters
void tims_config_set_smoothing_window(tims_config *cfg, uint32_t window);
void tims_config_set_centroiding_window(tims_config *cfg, uint32_t window);
void tims_config_set_calibration_tolerance(tims_config *cfg, double tolerance);
void tims_config_set_calibrate(tims_config *cfg, uint8_t enabled); // 0 = disabled, non-zero = enabled

// FrameWindowSplittingConfiguration setters
// (exact setters TBD — will be finalized during implementation by inspecting
//  timsrust 0.4.2's FrameWindowSplittingConfiguration fields. Note: the
//  UniformMobility variant takes an Option<Scan2ImConverter>, which may require
//  opening the dataset first to obtain the converter — this chicken-and-egg
//  constraint may limit which DIA splitting modes are configurable pre-open.)

// Open with config (existing tims_open remains for default config)
tims_status tims_open_with_config(
    const char *path,
    const tims_config *cfg,
    tims_dataset **out
);
```

Current `tims_open()` is unchanged and continues to use timsrust defaults.

## Rust-Side Architecture

### Changes to `TimsDataset` (dataset.rs)

- Add `frame_reader: FrameReader` — constructed at open time alongside `SpectrumReader`
- Add `mz_converter: Tof2MzConverter` and `im_converter: Scan2ImConverter` — from `MetadataReader::new()` at open time
- Add frame buffers: `tof_buf: Vec<u32>`, `int_buf_u32: Vec<u32>`, `scan_offset_buf: Vec<u64>` for single-frame handle-owned access
- Populate new `TimsFfiSpectrum` fields in `get_spectrum()` and `tims_get_spectra_by_rt()`
- `num_frames` field can be replaced by `frame_reader.len()`

### New file: `config.rs`

- `TimsFfiConfig` wrapper around `SpectrumReaderConfig`
- Setter methods mapping to individual config fields
- Used by `tims_open_with_config()` to build the `SpectrumReader`

### Changes to `types.rs`

- Add `TimsFfiFrame` repr(C) struct
- Extend `TimsFfiSpectrum` with new fields

### Changes to `lib.rs`

- New FFI exports for all new functions
- `tims_open_with_config()` passes `SpectrumReaderConfig` (including `FrameWindowSplittingConfiguration`) to the builder via `with_config()`. The builder internally resolves converter dependencies during `finalize()`.
- Frame functions delegate to `TimsDataset` methods
- Converter functions delegate to cached converters

### Stub mode (without `with_timsrust`)

All new functions get stub implementations:
- Frame functions return empty frames / zero counts
- Converters return identity (input cast to f64)
- Config functions create/free a dummy struct
- `tims_open_with_config` ignores config, behaves like `tims_open`

## New Function Summary

| Function | Category | Memory |
|---|---|---|
| `tims_get_frame` | Frame: single | Handle-owned |
| `tims_get_frames_by_level` | Frame: batch | Caller-owned (malloc) |
| `tims_free_frame_array` | Frame: cleanup | — |
| `tims_convert_tof_to_mz` | Converter: single | Return value |
| `tims_convert_scan_to_im` | Converter: single | Return value |
| `tims_convert_tof_to_mz_array` | Converter: batch | Caller-provided buffer |
| `tims_convert_scan_to_im_array` | Converter: batch | Caller-provided buffer |
| `tims_config_create` | Config: lifecycle | Returns Box'd |
| `tims_config_free` | Config: lifecycle | — |
| `tims_config_set_*` | Config: setters | — |
| `tims_open_with_config` | Config: open | — |

## Non-Goals

- No new error codes (existing `TimsFfiStatus` values suffice)
- No DIA-specific API (DDA/DIA handled uniformly through SpectrumReaderConfig)
- No thread-safety changes (same single-handle-single-thread model)
- No changes to existing function signatures or behavior
