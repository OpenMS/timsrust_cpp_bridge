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
    uint32_t index;               // native spectrum index
    double   isolation_width;     // isolation window width (0.0 if N/A)
    uint8_t  charge;              // precursor charge (0 = unknown)
    float    precursor_intensity; // precursor intensity (NaN = unknown)
    uint32_t frame_index;         // precursor's frame index (0 if N/A)
} tims_spectrum;
```

Sentinel values for optional fields: `0` for charge/frame_index, `NaN` for precursor_intensity, `0.0` for isolation_width. Keeps the struct flat and C-friendly.

### 2. Frame-Level Access

#### New type: `TimsFfiFrame`

```c
typedef struct tims_frame {
    uint32_t  index;          // frame index
    double    rt_seconds;     // retention time
    uint8_t   ms_level;       // 1=MS1, 2=MS2
    uint32_t  num_scans;      // number of scans
    uint32_t  num_peaks;      // total peaks (length of tof_indices & intensities)
    uint32_t *tof_indices;    // raw TOF indices, flat array
    uint32_t *intensities;    // raw intensities, flat array
    uint64_t *scan_offsets;   // per-scan offsets into flat arrays (length: num_scans + 1)
} tims_frame;
```

Raw indices are preserved (not converted to m/z) so callers can perform efficient discrete-domain operations like binning/summing on TOF indices before converting.

#### New functions

**Single-frame access (handle-owned buffers):**
```c
tims_status tims_get_frame(tims_dataset *ds, uint32_t index, tims_frame *out);
```
Buffers are owned by the dataset handle, valid until the next frame operation on that handle.

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
Runs `FrameReader::parallel_filter()` on the Rust side — C++ callers get rayon parallelism for free.

### 3. Converters

Methods on the dataset handle. `MetadataReader` is called at open time; `Tof2MzConverter` and `Scan2ImConverter` are cached inside `TimsDataset`.

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

Batch versions take caller-provided output buffers (no malloc — caller knows the size). Single-value versions return the result directly (converter is always valid once dataset is open).

### 4. Configurable Reader Construction

**Opaque config builder:**
```c
typedef struct tims_config tims_config;

tims_config *tims_config_create(void);
void         tims_config_free(tims_config *cfg);

// SpectrumProcessingParams setters
void tims_config_set_smoothing_window(tims_config *cfg, uint32_t window);
void tims_config_set_centroiding_window(tims_config *cfg, uint32_t window);

// FrameWindowSplittingConfiguration setters
// (exact setters TBD — will be finalized during implementation
//  by inspecting timsrust 0.4.2's FrameWindowSplittingConfiguration fields)

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
- Populate new `TimsFfiSpectrum` fields in `get_spectrum()`

### New file: `config.rs`

- `TimsFfiConfig` wrapper around `SpectrumReaderConfig`
- Setter methods mapping to individual config fields
- Used by `tims_open_with_config()` to build the `SpectrumReader`

### Changes to `types.rs`

- Add `TimsFfiFrame` repr(C) struct
- Extend `TimsFfiSpectrum` with new fields

### Changes to `lib.rs`

- New FFI exports for all new functions
- `tims_open_with_config()` uses the builder pattern: `SpectrumReader::build().with_path().with_config().finalize()`
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
| `tims_config_create` | Config: lifecycle | Returns malloc'd |
| `tims_config_free` | Config: lifecycle | — |
| `tims_config_set_*` | Config: setters | — |
| `tims_open_with_config` | Config: open | — |

## Non-Goals

- No new error codes (existing `TimsFfiStatus` values suffice)
- No DIA-specific API (DDA/DIA handled uniformly through SpectrumReaderConfig)
- No thread-safety changes (same single-handle-single-thread model)
- No changes to existing function signatures or behavior
