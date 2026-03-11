// src/lib.rs
mod dataset;
mod types;

use crate::dataset::TimsDataset;
use crate::types::{TimsFfiSpectrum, TimsFfiStatus, TimsFfiFileInfo, TimsFfiLevelStats};
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
pub extern "C" fn tims_num_frames(handle: *const tims_dataset) -> c_uint {
    if handle.is_null() { return 0; }
    let ds = unsafe { &(*handle).inner };
    ds.num_frames()
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

        // Fast RT lookup using the sorted RT index built at open time.
        // Binary search to the insertion point of rt_seconds, then expand
        // outward collecting candidates within the drift window, stopping
        // once we have enough spectra.
        let rt_idx = ds.rt_index();
        let take = std::cmp::max(0, n_spectra) as usize;

        // Find the nearest position in the sorted RT index
        let pivot = rt_idx.partition_point(|&(rt, _)| rt < rt_seconds);

        // Walk outward from pivot, interleaving left/right, until we have
        // `take` IM-filtered candidates.
        let mut candidates: Vec<(f64, usize)> = Vec::with_capacity(take * 2 + 4);
        let mut lo = pivot.saturating_sub(1);
        let mut hi = pivot;
        let total = rt_idx.len();

        // We scan at most 2*take*oversampling entries (drift filter may reject many)
        let max_scan = (take * 32).max(512).min(total);
        let mut scanned = 0;
        loop {
            if scanned >= max_scan { break; }
            let use_hi = hi < total && (lo == usize::MAX || {
                let dt_hi = if hi < total { (rt_idx[hi].0 - rt_seconds).abs() } else { f64::INFINITY };
                let dt_lo = if lo < total { (rt_idx[lo].0 - rt_seconds).abs() } else { f64::INFINITY };
                dt_hi <= dt_lo
            });
            let (check, advance_hi) = if use_hi { (hi, true) } else { (lo, false) };
            if check >= total { break; }
            let (rt, spec_idx) = rt_idx[check];
            // We need the IM of this spectrum — fetch cheaply via reader.get()
            // only if drift filtering is active (drift_end < f64::INFINITY).
            let im_ok = if drift_end < 1e15 {
                if let Ok(spec) = ds.reader.get(spec_idx) {
                    spec.precursor.map(|p| p.im >= drift_start && p.im <= drift_end).unwrap_or(false)
                } else {
                    false
                }
            } else {
                true
            };
            if im_ok {
                let dist = (rt - rt_seconds).abs();
                candidates.push((dist, spec_idx));
            }
            if advance_hi { hi += 1; } else { lo = lo.wrapping_sub(1); }
            if lo == usize::MAX && hi >= total { break; }
            scanned += 1;
        }
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let take_n = std::cmp::min(take, candidates.len());

        if take_n == 0 {
            unsafe { *out_count = 0; *out_specs = std::ptr::null_mut(); }
            return TimsFfiStatus::Ok;
        }

        // Allocate array of TimsFfiSpectrum via malloc
        let arr_ptr = (unsafe { malloc(take_n * mem::size_of::<TimsFfiSpectrum>()) }) as *mut TimsFfiSpectrum;
        if arr_ptr.is_null() { return TimsFfiStatus::Internal; }

        // For each chosen spectrum, copy data and allocate mz/int arrays via malloc
        for idx in 0..take_n {
            let spec_idx = candidates[idx].1;
            let spec = match ds.reader.get(spec_idx) {
                Ok(s) => s,
                Err(_) => { unsafe { free(arr_ptr as *mut libc::c_void); } ; return TimsFfiStatus::Internal; }
            };
            let n = spec.len();
            let mz_ptr = if n == 0 { std::ptr::null_mut() } else { (unsafe { malloc(n * mem::size_of::<f32>()) }) as *mut f32 };
            let int_ptr = if n == 0 { std::ptr::null_mut() } else { (unsafe { malloc(n * mem::size_of::<f32>()) }) as *mut f32 };
            if (n > 0) && (mz_ptr.is_null() || int_ptr.is_null()) {
                for j in 0..idx {
                    unsafe {
                        let old = arr_ptr.add(j).read();
                        if !old.mz.is_null() { free(old.mz as *mut libc::c_void); }
                        if !old.intensity.is_null() { free(old.intensity as *mut libc::c_void); }
                    }
                }
                unsafe { free(arr_ptr as *mut libc::c_void); }
                return TimsFfiStatus::Internal;
            }
            for i in 0..n {
                if !mz_ptr.is_null() { unsafe { *mz_ptr.add(i) = spec.mz_values[i] as f32; } }
                if !int_ptr.is_null() { unsafe { *int_ptr.add(i) = spec.intensities[i] as f32; } }
            }
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
            unsafe { arr_ptr.add(idx).write(out_spec); }
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

/// Collect aggregate file statistics in a single pass over all spectra.
/// Uses timsrust's parallel get_all() under the hood so is reasonably fast.
/// Returns TIMSFFI_OK and fills *out on success.
#[no_mangle]
pub extern "C" fn tims_file_info(
    handle: *mut tims_dataset,
    out: *mut TimsFfiFileInfo,
) -> TimsFfiStatus {
    if handle.is_null() || out.is_null() {
        return TimsFfiStatus::Internal;
    }

    let ds = unsafe { &mut (*handle).inner };

    let mut info = TimsFfiFileInfo {
        num_frames:     ds.num_frames(),
        num_spectra_ms2: ds.len(),
        total_peaks:    0,
        ms1: TimsFfiLevelStats::empty(),
        ms2: TimsFfiLevelStats::empty(),
        wall_ms: 0.0,
    };

    #[cfg(feature = "with_timsrust")]
    {
        use std::time::Instant;
        let t0 = Instant::now();

        // get_all() uses rayon internally — parallel decompression
        let all = ds.reader.get_all();

        for result in &all {
            let spec = match result { Ok(s) => s, Err(_) => continue };
            let ms_level: u8 = if spec.precursor.is_some() { 2 } else { 1 };
            let stat = if ms_level == 1 { &mut info.ms1 } else { &mut info.ms2 };

            stat.count += 1;
            stat.total_peaks += spec.mz_values.len() as u64;
            info.total_peaks  += spec.mz_values.len() as u64;

            if let Some(prec) = spec.precursor {
                let rt = prec.rt;
                let im = prec.im;
                if rt < stat.rt_min { stat.rt_min = rt; }
                if rt > stat.rt_max { stat.rt_max = rt; }
                if im < stat.im_min { stat.im_min = im; }
                if im > stat.im_max { stat.im_max = im; }
            }
            for &mz in &spec.mz_values {
                if mz < stat.mz_min { stat.mz_min = mz; }
                if mz > stat.mz_max { stat.mz_max = mz; }
            }
            for &inten in &spec.intensities {
                if inten < stat.intensity_min { stat.intensity_min = inten; }
                if inten > stat.intensity_max { stat.intensity_max = inten; }
            }
        }

        info.ms1.finalize();
        info.ms2.finalize();
        info.wall_ms = t0.elapsed().as_secs_f64() * 1000.0;
    }

    unsafe { *out = info; }
    TimsFfiStatus::Ok
}
