/* include/timsffi.h */

#ifndef TIMSFFI_H
#define TIMSFFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct tims_dataset tims_dataset;

typedef enum {
    TIMSFFI_OK = 0,
    TIMSFFI_ERR_INVALID_UTF8 = 1,
    TIMSFFI_ERR_OPEN_FAILED = 2,
    TIMSFFI_ERR_INDEX_OOB   = 3,
    TIMSFFI_ERR_INTERNAL    = 255
} timsffi_status;

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

typedef struct {
    double mz_lower;
    double mz_upper;
    double mz_center;
    double im_lower;
    double im_upper;
    uint8_t is_ms1;
} tims_swath_window;

typedef struct {
    uint32_t index;
    double   rt_seconds;
    uint8_t  ms_level;           /* 1=MS1, 2=MS2, 0=Unknown */
    uint32_t num_scans;
    uint32_t num_peaks;          /* total peaks (length of tof_indices & intensities) */
    const uint32_t* tof_indices; /* raw TOF indices, flat array */
    const uint32_t* intensities; /* raw intensities, flat array */
    const uint64_t* scan_offsets;/* per-scan offsets (length: num_scans + 1) */
} tims_frame;

/* functions: tims_open, tims_close, tims_num_spectra, tims_get_spectrum, ... */
/* Function prototypes (C ABI)
 * Note: mz/intensity pointers returned from `tims_get_spectrum` currently
 * point to no data when num_peaks == 0. In the single-spectrum API these
 * pointers may point into internal buffers owned by the handle; callers
 * must not dereference them after calling other functions on the same
 * handle or after tims_close(handle).
 *
 * The multi-spectrum API (`tims_get_spectra_by_rt`) allocates an array of
 * `tims_spectrum` and per-spectrum mz/intensity arrays on the caller's
 * behalf using the C heap. Callers MUST free the returned data with
 * `tims_free_spectrum_array(handle, specs, count)` which will free the
 * per-spectrum buffers and the array itself.
 */

/* Open dataset at path. On success returns TIMSFFI_OK and sets *out to a
 * newly allocated handle. Caller must call tims_close on the handle.
 */
timsffi_status tims_open(const char* path, tims_dataset** out);

/* Close and free handle */
void tims_close(tims_dataset* handle);

/* Number of spectra (0 if handle is NULL).
 * NOTE: For DIA-PASEF datasets this returns the number of expanded MS2 DIA
 * spectra (one per quadrupole isolation window × frame), NOT the total number
 * of raw LC frames. Use tims_num_frames() to obtain the raw frame count which
 * includes MS1 frames and matches the number of "spectra" reported by mzML
 * converters.
 */
unsigned int tims_num_spectra(const tims_dataset* handle);

/* Total number of raw LC frames (MS1 + MS2) in the acquisition.
 * For a DIA-PASEF run this is typically: MS1 frames + MS2 PASEF frames.
 * An mzML conversion of the same run will have roughly this many spectra.
 */
unsigned int tims_num_frames(const tims_dataset* handle);

/* Fill out a spectrum structure for the given index. Returns status code.
 * If the function returns TIMSFFI_OK then `out` is populated. If
 * out->num_peaks > 0 then mz/intensity point to data (see ownership note
 * above). For the current minimal implementation num_peaks will be 0.
 */
timsffi_status tims_get_spectrum(tims_dataset* handle, unsigned int index, tims_spectrum* out_spec);

/* Retrieve swath windows (DIA isolation windows) if available. On success
 * returns TIMSFFI_OK and sets *out_count and *out_windows to a newly
 * allocated array (caller must free with tims_free_swath_windows).
 */
timsffi_status tims_get_swath_windows(tims_dataset* handle, unsigned int* out_count, tims_swath_window** out_windows);

/* Free swath windows allocated by tims_get_swath_windows. */
void tims_free_swath_windows(tims_dataset* handle, tims_swath_window* windows);

/* Retrieve up to `n_spectra` spectra near rt_seconds within the given
 * drift (ion mobility) window [drift_start, drift_end]. Returns an
 * allocated array in *out_specs and sets *out_count. Caller must free
 * with tims_free_spectrum_array(handle, specs, count).
 */
timsffi_status tims_get_spectra_by_rt(tims_dataset* handle, double rt_seconds, int n_spectra, double drift_start, double drift_end, unsigned int* out_count, tims_spectrum** out_specs);

/* Free spectra previously returned by tims_get_spectra_by_rt. Frees each
 * per-spectrum mz/intensity buffer and then the array itself.
 */
void tims_free_spectrum_array(tims_dataset* handle, tims_spectrum* specs, unsigned int count);

/* Retrieve the last error string. If handle is NULL, returns the last
 * global error (e.g., from a failed tims_open). Copies up to buf_len-1
 * bytes and always NUL-terminates when buf_len > 0. Returns TIMSFFI_OK on
 * success, or TIMSFFI_ERR_INTERNAL on internal failure.
 */
timsffi_status tims_get_last_error(tims_dataset* handle, char* buf, unsigned int buf_len);

/* -------------------------------------------------------------------------
 * File-level aggregate statistics (cf. OpenMS FileInfo output)
 * ------------------------------------------------------------------------- */

/* Per-MS-level statistics. */
typedef struct {
    uint32_t count;          /* number of spectra at this MS level */
    uint64_t total_peaks;
    double rt_min, rt_max;   /* retention time range (seconds) */
    double mz_min, mz_max;   /* m/z range */
    double im_min, im_max;   /* ion mobility range */
    double intensity_min, intensity_max;
} tims_level_stats;

/* Aggregate statistics for the whole file. */
typedef struct {
    uint32_t num_frames;      /* total raw LC frames (MS1 + MS2 combined) */
    uint32_t num_spectra_ms2; /* expanded DIA/DDA MS2 spectra */
    uint64_t total_peaks;
    tims_level_stats ms1;
    tims_level_stats ms2;
    double wall_ms;           /* wall time to collect stats (ms) */
} tims_file_info_t;

/* Collect aggregate file statistics in a single parallel pass.
 * Returns TIMSFFI_OK and fills *out on success.
 * Note: MS1 stats will only be populated if MS1 spectra are present in the
 * SpectrumReader view (DIA-PASEF datasets expose MS2 windows only; use
 * num_frames for the raw LC frame count which includes MS1 frames).
 */
timsffi_status tims_file_info(tims_dataset* handle, tims_file_info_t* out);

/* -------------------------------------------------------------------------
 * Frame-level access
 * ------------------------------------------------------------------------- */

/* Fill out a frame structure for the given index. Returns status code.
 * Pointers in the output point to internal buffers owned by the handle;
 * valid until the next operation on the same handle or tims_close().
 */
timsffi_status tims_get_frame(tims_dataset* handle, unsigned int index, tims_frame* out_frame);

/* Retrieve all frames at the given MS level (1 or 2). Returns an
 * allocated array in *out_frames and sets *out_count. Caller must free
 * with tims_free_frame_array(handle, frames, count). Invalid ms_level
 * returns an empty array with TIMSFFI_OK.
 */
timsffi_status tims_get_frames_by_level(tims_dataset* handle, uint8_t ms_level, unsigned int* out_count, tims_frame** out_frames);

/* Free frames previously returned by tims_get_frames_by_level. Frees each
 * per-frame tof_indices/intensities/scan_offsets buffer and then the array.
 */
void tims_free_frame_array(tims_dataset* handle, tims_frame* frames, unsigned int count);


#ifdef __cplusplus
}
#endif

#endif /* TIMSFFI_H */

