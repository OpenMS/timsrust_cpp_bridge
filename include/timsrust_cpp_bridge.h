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
 * point to no data when num_peaks == 0. In future implementations these
 * may point into internal buffers owned by the handle; callers must not
 * dereference them after calling other functions on the same handle or
 * after tims_close(handle).
 */

/* Open dataset at path. On success returns TIMSFFI_OK and sets *out to a
 * newly allocated handle. Caller must call tims_close on the handle.
 */
int tims_open(const char* path, tims_dataset** out);

/* Close and free handle */
void tims_close(tims_dataset* handle);

/* Number of spectra (0 if handle is NULL) */
unsigned int tims_num_spectra(const tims_dataset* handle);

/* Fill out a spectrum structure for the given index. Returns status code.
 * If the function returns TIMSFFI_OK then `out` is populated. If
 * out->num_peaks > 0 then mz/intensity point to data (see ownership note
 * above). For the current minimal implementation num_peaks will be 0.
 */
int tims_get_spectrum(tims_dataset* handle, unsigned int index, tims_spectrum* out_spec);


#ifdef __cplusplus
}
#endif

#endif /* TIMSFFI_H */

