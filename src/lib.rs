// src/lib.rs
mod dataset;
mod types;
mod errors; // optional for error mapping

use crate::dataset::TimsDataset;
use crate::types::{TimsFfiSpectrum, TimsFfiStatus};
use std::ffi::CStr;
use std::os::raw::{c_char, c_uint};
use std::os::raw::{c_int, c_double};
use libc::{malloc, free};
use std::mem;
use std::ffi::CString;
use std::thread;
use std::sync::mpsc;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use crate::types::TIMSFFI_MAX_ERROR_LEN;

static LAST_ERROR: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

#[repr(C)]
pub struct tims_dataset {
    inner: TimsDataset,
}

#[no_mangle]
pub extern "C" fn tims_open(
    path: *const c_char,
    out_handle: *mut *mut tims_dataset,
) -> TimsFfiStatus {
    if path.is_null() || out_handle.is_null() {
        return TimsFfiStatus::Internal;
    }
    let cstr = unsafe { CStr::from_ptr(path) };
    let path_str = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return TimsFfiStatus::InvalidUtf8,
    };
    // Basic pre-check: ensure the path exists to avoid timsrust internals
    // attempting I/O that may panic in other threads or via unwraps.
    if std::fs::metadata(path_str).is_err() {
        if let Ok(mut g) = LAST_ERROR.lock() { *g = Some(format!("path not found: {}", path_str)); }
        return TimsFfiStatus::OpenFailed;
    }

    // Perform the open inside a fresh thread and receive the result via a
    // channel. If the thread panics (or returns an Err) we capture the
    // stringified error (or panic message) and store it in LAST_ERROR so
    // callers can fetch it via tims_get_last_error.
    let path_owned = path_str.to_string();
    let (tx, rx) = mpsc::channel();
    let thr = thread::spawn(move || {
        let c = CString::new(path_owned).unwrap();
        let res = std::panic::catch_unwind(|| TimsDataset::open(c.as_c_str()));
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

    // Join and read the result
    let got = match thr.join() {
        Ok(_) => rx.recv(),
        Err(_) => Err(std::sync::mpsc::RecvError),
    };

    match got {
        Ok(Ok(inner)) => {
            let boxed = Box::new(tims_dataset { inner });
            unsafe { *out_handle = Box::into_raw(boxed); }
            // clear global last error
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

#[no_mangle]
pub extern "C" fn tims_close(handle: *mut tims_dataset) {
    if handle.is_null() { return; }
    unsafe { drop(Box::from_raw(handle)); }
}

#[no_mangle]
pub extern "C" fn tims_num_spectra(handle: *const tims_dataset) -> c_uint {
    if handle.is_null() { return 0; }
    let ds = unsafe { &(*handle).inner };
    ds.len()
}

#[no_mangle]
pub extern "C" fn tims_get_spectrum(
    handle: *mut tims_dataset,
    index: c_uint,
    out_spec: *mut TimsFfiSpectrum,
) -> TimsFfiStatus {
    if handle.is_null() || out_spec.is_null() {
        return TimsFfiStatus::Internal;
    }
    let ds = unsafe { &mut (*handle).inner };
    let out = unsafe { &mut *out_spec };
    match ds.get_spectrum(index, out) {
        Ok(()) => TimsFfiStatus::Ok,
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn tims_get_swath_windows(
    handle: *mut tims_dataset,
    out_count: *mut c_uint,
    out_windows: *mut *mut crate::types::TimsFfiSwathWindow,
) -> TimsFfiStatus {
    if handle.is_null() || out_count.is_null() || out_windows.is_null() {
        return TimsFfiStatus::Internal;
    }
    let ds = unsafe { &(*handle).inner };
    match ds.get_swath_windows() {
        Ok(windows) => {
            let n = windows.len();
            if n == 0 {
                unsafe { *out_count = 0; *out_windows = std::ptr::null_mut(); }
                return TimsFfiStatus::Ok;
            }
            let size = n * mem::size_of::<crate::types::TimsFfiSwathWindow>();
            let ptr = unsafe { malloc(size) } as *mut crate::types::TimsFfiSwathWindow;
            if ptr.is_null() {
                return TimsFfiStatus::Internal;
            }
            for i in 0..n {
                unsafe { ptr.add(i).write(windows[i].clone()); }
            }
            unsafe { *out_count = n as c_uint; *out_windows = ptr; }
            TimsFfiStatus::Ok
        }
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn tims_free_swath_windows(
    _handle: *mut tims_dataset,
    windows: *mut crate::types::TimsFfiSwathWindow,
) {
    if windows.is_null() { return; }
    unsafe { free(windows as *mut libc::c_void); }
}

#[no_mangle]
pub extern "C" fn tims_get_last_error(
    handle: *mut tims_dataset,
    buf: *mut libc::c_char,
    buf_len: c_uint,
) -> TimsFfiStatus {
    if buf.is_null() || buf_len == 0 {
        return TimsFfiStatus::Internal;
    }
    // Determine source: per-handle last_error if handle provided, otherwise global
    let msg_opt = if !handle.is_null() {
        let ds = unsafe { &(*handle).inner };
        ds.last_error.clone()
    } else {
        if let Ok(g) = LAST_ERROR.lock() { g.clone() } else { None }
    };

    if let Some(msg) = msg_opt {
        // copy up to buf_len-1 bytes
        let bytes = msg.as_bytes();
        let copy_len = std::cmp::min(bytes.len(), (buf_len as usize).saturating_sub(1));
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
            *buf.add(copy_len) = 0;
        }
        TimsFfiStatus::Ok
    } else {
        // empty string
        unsafe { *buf = 0; }
        TimsFfiStatus::Ok
    }
}

#[no_mangle]
pub extern "C" fn tims_get_spectra_by_rt(
    handle: *mut tims_dataset,
    rt_seconds: c_double,
    n_spectra: c_int,
    drift_start: c_double,
    drift_end: c_double,
    out_count: *mut c_uint,
    out_specs: *mut *mut TimsFfiSpectrum,
) -> TimsFfiStatus {
    // This API is only available when built with the real timsrust feature.
    #[cfg(not(feature = "with_timsrust"))]
    {
        // Feature not enabled: return empty result
        if out_count.is_null() || out_specs.is_null() { return TimsFfiStatus::Internal; }
        unsafe { *out_count = 0; *out_specs = std::ptr::null_mut(); }
        return TimsFfiStatus::Ok;
    }

    #[cfg(feature = "with_timsrust")]
    {
        if handle.is_null() || out_count.is_null() || out_specs.is_null() {
            return TimsFfiStatus::Internal;
        }
        let ds = unsafe { &mut (*handle).inner };

        // Collect candidate spectra indices within drift window
        let mut candidates: Vec<(f64, usize)> = Vec::new();
        let total = ds.len() as usize;
        for i in 0..total {
            if let Ok(spec) = ds.reader.get(i) {
                if let Some(prec) = spec.precursor {
                    if prec.im >= drift_start && prec.im <= drift_end {
                        let dist = (prec.rt - rt_seconds).abs();
                        candidates.push((dist, i));
                    }
                }
            }
        }
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let take = std::cmp::max(0, n_spectra) as usize;
        let take_n = std::cmp::min(take, candidates.len());

        if take_n == 0 {
            unsafe { *out_count = 0; *out_specs = std::ptr::null_mut(); }
            return TimsFfiStatus::Ok;
        }

        // Allocate array of TimsFfiSpectrum via malloc
        let spec_size = mem::size_of::<TimsFfiSpectrum>();
        let arr_size = take_n * spec_size;
        let arr_ptr = unsafe { malloc(arr_size) } as *mut TimsFfiSpectrum;
        if arr_ptr.is_null() { return TimsFfiStatus::Internal; }

        // For each chosen spectrum, copy data and allocate mz/int arrays via malloc
        for idx in 0..take_n {
            let spec_idx = candidates[idx].1;
            let spec = match ds.reader.get(spec_idx) {
                Ok(s) => s,
                Err(_) => { unsafe { free(arr_ptr as *mut libc::c_void); } ; return TimsFfiStatus::Internal; }
            };
            let n = spec.len();
            // allocate mz and intensity arrays of f32
            let mz_bytes = n * mem::size_of::<f32>();
            let int_bytes = n * mem::size_of::<f32>();
            let mz_ptr = if n == 0 { std::ptr::null_mut() } else { (unsafe { malloc(mz_bytes) }) as *mut f32 };
            let int_ptr = if n == 0 { std::ptr::null_mut() } else { (unsafe { malloc(int_bytes) }) as *mut f32 };
            if (n > 0) && (mz_ptr.is_null() || int_ptr.is_null()) {
                // free allocated so far
                for j in 0..idx {
                    unsafe {
                        let old = arr_ptr.add(j);
                        let old_spec = old.read();
                        if !old_spec.mz.is_null() {
                            free(old_spec.mz as *mut libc::c_void);
                        }
                        if !old_spec.intensity.is_null() {
                            free(old_spec.intensity as *mut libc::c_void);
                        }
                    }
                }
                unsafe { free(arr_ptr as *mut libc::c_void); }
                return TimsFfiStatus::Internal;
            }
            // copy data
            for i in 0..n {
                if !mz_ptr.is_null() { unsafe { *mz_ptr.add(i) = spec.mz_values[i] as f32; } }
                if !int_ptr.is_null() { unsafe { *int_ptr.add(i) = spec.intensities[i] as f32; } }
            }
            // construct TimsFfiSpectrum
            let mut out = TimsFfiSpectrum {
                rt_seconds: spec.precursor.map(|p| p.rt).unwrap_or(0.0),
                precursor_mz: spec.precursor.map(|p| p.mz).unwrap_or(0.0),
                ms_level: if spec.precursor.is_some() { 2 } else { 1 },
                num_peaks: n as u32,
                mz: if mz_ptr.is_null() { std::ptr::null() } else { mz_ptr as *const f32 },
                intensity: if int_ptr.is_null() { std::ptr::null() } else { int_ptr as *const f32 },
                im: spec.precursor.map(|p| p.im).unwrap_or(0.0),
            };
            unsafe { arr_ptr.add(idx).write(out); }
        }

        unsafe { *out_count = take_n as c_uint; *out_specs = arr_ptr; }
        TimsFfiStatus::Ok
    }
}

#[no_mangle]
pub extern "C" fn tims_free_spectrum_array(
    _handle: *mut tims_dataset,
    specs: *mut TimsFfiSpectrum,
    count: c_uint,
) {
    if specs.is_null() { return; }
    let n = count as usize;
    // free per-spectrum mz/int arrays
    for i in 0..n {
        unsafe {
            let sp = specs.add(i).read();
            if !sp.mz.is_null() { free(sp.mz as *mut libc::c_void); }
            if !sp.intensity.is_null() { free(sp.intensity as *mut libc::c_void); }
        }
    }
    // free the array itself
    unsafe { free(specs as *mut libc::c_void); }
}

