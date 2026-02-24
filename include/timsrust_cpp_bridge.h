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
} tims_spectrum;

typedef struct {
    double mz_lower;
    double mz_upper;
    double mz_center;
    double im_lower;
    double im_upper;
    uint8_t is_ms1;
} tims_swath_window;

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

/* Number of spectra (0 if handle is NULL) */
unsigned int tims_num_spectra(const tims_dataset* handle);

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


#ifdef __cplusplus
}
#endif

#endif /* TIMSFFI_H */

