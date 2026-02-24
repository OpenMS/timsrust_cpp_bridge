// src/lib.rs
mod dataset;
mod types;
mod errors; // optional for error mapping

use crate::dataset::TimsDataset;
use crate::types::{TimsFfiSpectrum, TimsFfiStatus};
use std::ffi::CStr;
use std::os::raw::{c_char, c_uint};

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
    match TimsDataset::open(cstr) {
        Ok(inner) => {
            let boxed = Box::new(tims_dataset { inner });
            unsafe {
                *out_handle = Box::into_raw(boxed);
            }
            TimsFfiStatus::Ok
        }
        Err(e) => e,
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

