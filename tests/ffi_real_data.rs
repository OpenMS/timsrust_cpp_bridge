// tests/ffi_real_data.rs
//
// Real-data integration tests that run against actual Bruker .d datasets.
// Gated behind environment variables:
//   TIMSRUST_TEST_DATA_DDA — path to a DDA .d dataset
//   TIMSRUST_TEST_DATA_DIA — path to a DIA-PASEF .d dataset
//
// When the corresponding env var is not set, tests print a skip message
// and return without failure.

mod common;
use common::*;
use std::ptr;

// ---- Env-var gating macros ----

macro_rules! require_dda {
    () => {
        match dda_path() {
            Some(p) => p,
            None => {
                eprintln!("TIMSRUST_TEST_DATA_DDA not set, skipping");
                return;
            }
        }
    };
}

macro_rules! require_dia {
    () => {
        match dia_path() {
            Some(p) => p,
            None => {
                eprintln!("TIMSRUST_TEST_DATA_DIA not set, skipping");
                return;
            }
        }
    };
}

// ============================================================
// DDA Tests
// ============================================================

#[test]
fn dda_open_succeeds() {
    let path = require_dda!();
    let handle = open_real(&path);
    unsafe { tims_close(handle) };
}

#[test]
fn dda_num_spectra_positive() {
    let path = require_dda!();
    let handle = open_real(&path);
    let n = unsafe { tims_num_spectra(handle as *const _) };
    assert!(n > 0, "DDA dataset should have > 0 spectra, got {n}");
    unsafe { tims_close(handle) };
}

#[test]
fn dda_num_frames_positive() {
    let path = require_dda!();
    let handle = open_real(&path);
    let num_frames = unsafe { tims_num_frames(handle as *const _) };
    let num_spectra = unsafe { tims_num_spectra(handle as *const _) };
    assert!(num_frames > 0, "DDA dataset should have > 0 frames, got {num_frames}");
    assert!(
        num_frames <= num_spectra,
        "num_frames ({num_frames}) should be <= num_spectra ({num_spectra})"
    );
    unsafe { tims_close(handle) };
}

#[test]
fn dda_get_spectrum_0() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let spec = unsafe { spec.assume_init() };

    assert!(spec.num_peaks > 0, "spectrum 0 should have > 0 peaks, got {}", spec.num_peaks);
    assert!(!spec.mz.is_null(), "mz pointer should be non-null");
    assert!(!spec.intensity.is_null(), "intensity pointer should be non-null");
    assert!(
        spec.ms_level == 1 || spec.ms_level == 2,
        "ms_level should be 1 or 2, got {}",
        spec.ms_level
    );

    unsafe { tims_close(handle) };
}

#[test]
fn dda_spectrum_mz_sorted() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let spec = unsafe { spec.assume_init() };

    let n = spec.num_peaks as usize;
    if n > 1 {
        let mz_slice = unsafe { std::slice::from_raw_parts(spec.mz, n) };
        for i in 1..n {
            assert!(
                mz_slice[i] >= mz_slice[i - 1],
                "mz values should be sorted: mz[{}]={} < mz[{}]={}",
                i,
                mz_slice[i],
                i - 1,
                mz_slice[i - 1]
            );
        }
    }

    unsafe { tims_close(handle) };
}

#[test]
fn dda_spectrum_intensity_non_negative() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(handle, 0, spec.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let spec = unsafe { spec.assume_init() };

    let n = spec.num_peaks as usize;
    if n > 0 {
        let int_slice = unsafe { std::slice::from_raw_parts(spec.intensity, n) };
        for (i, &val) in int_slice.iter().enumerate() {
            assert!(
                val >= 0.0,
                "intensity[{i}] should be >= 0.0, got {val}"
            );
        }
    }

    unsafe { tims_close(handle) };
}

#[test]
fn dda_spectrum_metadata_fields() {
    let path = require_dda!();
    let handle = open_real(&path);

    let num_spectra = unsafe { tims_num_spectra(handle as *const _) };
    let scan_limit = std::cmp::min(num_spectra, 100);

    let mut found_ms2 = false;
    for idx in 0..scan_limit {
        let mut spec = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
        let status = unsafe { tims_get_spectrum(handle, idx, spec.as_mut_ptr()) };
        assert_status(status, TIMSFFI_OK);
        let spec = unsafe { spec.assume_init() };

        if spec.ms_level == 2 {
            found_ms2 = true;
            assert!(
                spec.frame_index != u32::MAX,
                "MS2 spectrum {idx} should have a valid frame_index, got u32::MAX"
            );
            assert!(
                spec.isolation_width >= 0.0,
                "MS2 spectrum {idx} isolation_width should be >= 0.0, got {}",
                spec.isolation_width
            );
            assert!(
                spec.isolation_mz >= 0.0,
                "MS2 spectrum {idx} isolation_mz should be >= 0.0, got {}",
                spec.isolation_mz
            );
        }
    }

    // DDA datasets should contain at least some MS2 spectra in the first 100
    assert!(found_ms2, "expected at least one MS2 spectrum in first {scan_limit} spectra");

    unsafe { tims_close(handle) };
}

#[test]
fn dda_get_frame_0() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let frame = unsafe { frame.assume_init() };

    assert!(frame.num_peaks > 0, "frame 0 should have > 0 peaks, got {}", frame.num_peaks);
    assert!(frame.num_scans > 0, "frame 0 should have > 0 scans, got {}", frame.num_scans);

    unsafe { tims_close(handle) };
}

#[test]
fn dda_frame_scan_offsets_monotonic() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut frame = std::mem::MaybeUninit::<TimsFfiFrame>::zeroed();
    let status = unsafe { tims_get_frame(handle, 0, frame.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let frame = unsafe { frame.assume_init() };

    if frame.num_scans > 0 {
        let offsets_len = (frame.num_scans + 1) as usize;
        let offsets = unsafe { std::slice::from_raw_parts(frame.scan_offsets, offsets_len) };

        for i in 1..offsets_len {
            assert!(
                offsets[i] >= offsets[i - 1],
                "scan_offsets should be monotonic: offsets[{i}]={} < offsets[{}]={}",
                offsets[i],
                i - 1,
                offsets[i - 1]
            );
        }

        assert_eq!(
            offsets[offsets_len - 1],
            frame.num_peaks as u64,
            "last scan_offset ({}) should equal num_peaks ({})",
            offsets[offsets_len - 1],
            frame.num_peaks
        );
    }

    unsafe { tims_close(handle) };
}

#[test]
fn dda_get_frames_by_level_ms1() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut count: u32 = 0;
    let mut frames: *mut TimsFfiFrame = ptr::null_mut();
    let status = unsafe { tims_get_frames_by_level(handle, 1, &mut count, &mut frames) };
    assert_status(status, TIMSFFI_OK);
    assert!(count > 0, "DDA dataset should have > 0 MS1 frames, got {count}");
    assert!(!frames.is_null(), "frames pointer should be non-null");

    // First frame should be MS1
    let first = unsafe { *frames };
    assert_eq!(first.ms_level, 1, "first MS1 frame should have ms_level=1, got {}", first.ms_level);

    unsafe { tims_free_frame_array(handle, frames, count) };
    unsafe { tims_close(handle) };
}

#[test]
fn dda_get_spectra_by_rt() {
    let path = require_dda!();
    let handle = open_real(&path);

    // Get file info to determine mid RT
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };
    let mid_rt = (info.ms2.rt_min + info.ms2.rt_max) / 2.0;

    // Query spectra near mid RT, no IM filter (wide range)
    let mut count: u32 = 0;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, mid_rt, 3, 0.0, 1e15, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);
    assert!(count > 0, "should find > 0 spectra near mid RT ({mid_rt})");
    assert!(!specs.is_null(), "specs pointer should be non-null");

    // First returned spectrum's RT should be within 60s of requested
    let first = unsafe { *specs };
    let rt_diff = (first.rt_seconds - mid_rt).abs();
    assert!(
        rt_diff <= 60.0,
        "first spectrum RT ({}) should be within 60s of requested RT ({mid_rt}), diff={rt_diff}",
        first.rt_seconds
    );

    unsafe { tims_free_spectrum_array(handle, specs, count) };
    unsafe { tims_close(handle) };
}

#[test]
fn dda_converters_positive_finite() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mz = unsafe { tims_convert_tof_to_mz(handle as *const _, 100) };
    assert!(mz.is_finite(), "tof_to_mz(100) should be finite, got {mz}");
    assert!(mz > 0.0, "tof_to_mz(100) should be positive, got {mz}");

    let im = unsafe { tims_convert_scan_to_im(handle as *const _, 100) };
    assert!(im.is_finite(), "scan_to_im(100) should be finite, got {im}");
    assert!(im > 0.0, "scan_to_im(100) should be positive, got {im}");

    unsafe { tims_close(handle) };
}

#[test]
fn dda_converter_array_matches_scalar() {
    let path = require_dda!();
    let handle = open_real(&path);

    let tof_indices: [u32; 3] = [50, 100, 500];
    let mut mz_out: [f64; 3] = [0.0; 3];
    let status = unsafe {
        tims_convert_tof_to_mz_array(
            handle as *const _,
            tof_indices.as_ptr(),
            3,
            mz_out.as_mut_ptr(),
        )
    };
    assert_status(status, TIMSFFI_OK);

    for (i, &tof) in tof_indices.iter().enumerate() {
        let scalar = unsafe { tims_convert_tof_to_mz(handle as *const _, tof) };
        assert_eq!(
            mz_out[i], scalar,
            "tof_to_mz array[{i}] ({}) should match scalar ({scalar}) for tof={tof}",
            mz_out[i]
        );
    }

    let scan_indices: [u32; 3] = [50, 100, 500];
    let mut im_out: [f64; 3] = [0.0; 3];
    let status = unsafe {
        tims_convert_scan_to_im_array(
            handle as *const _,
            scan_indices.as_ptr(),
            3,
            im_out.as_mut_ptr(),
        )
    };
    assert_status(status, TIMSFFI_OK);

    for (i, &scan) in scan_indices.iter().enumerate() {
        let scalar = unsafe { tims_convert_scan_to_im(handle as *const _, scan) };
        assert_eq!(
            im_out[i], scalar,
            "scan_to_im array[{i}] ({}) should match scalar ({scalar}) for scan={scan}",
            im_out[i]
        );
    }

    unsafe { tims_close(handle) };
}

#[test]
fn dda_file_info_populated() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };

    assert!(info.total_peaks > 0, "total_peaks should be > 0, got {}", info.total_peaks);
    assert!(
        info.ms2.rt_min < info.ms2.rt_max,
        "ms2.rt_min ({}) should be < ms2.rt_max ({})",
        info.ms2.rt_min,
        info.ms2.rt_max
    );
    assert!(
        info.ms2.mz_min > 0.0,
        "ms2.mz_min should be > 0.0, got {}",
        info.ms2.mz_min
    );

    unsafe { tims_close(handle) };
}

#[test]
fn dda_swath_windows() {
    let path = require_dda!();
    let handle = open_real(&path);

    let mut count: u32 = 0;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(handle, &mut count, &mut windows) };
    assert_status(status, TIMSFFI_OK);

    // DDA datasets may have 0 swath windows — that is acceptable
    if count > 0 {
        assert!(!windows.is_null(), "windows pointer should be non-null when count > 0");
        unsafe { tims_free_swath_windows(handle, windows) };
    }

    unsafe { tims_close(handle) };
}

// ============================================================
// DIA Tests
// ============================================================

#[test]
fn dia_open_succeeds() {
    let path = require_dia!();
    let handle = open_real(&path);
    unsafe { tims_close(handle) };
}

#[test]
fn dia_spectra_much_more_than_frames() {
    let path = require_dia!();
    let handle = open_real(&path);

    let num_spectra = unsafe { tims_num_spectra(handle as *const _) };
    let num_frames = unsafe { tims_num_frames(handle as *const _) };

    assert!(
        num_spectra > num_frames * 2,
        "DIA dataset: num_spectra ({num_spectra}) should be > 2 * num_frames ({num_frames})"
    );

    unsafe { tims_close(handle) };
}

#[test]
fn dia_get_spectra_by_rt_with_im_filter() {
    let path = require_dia!();
    let handle = open_real(&path);

    // Get file info to determine mid RT and mid IM
    let mut info = std::mem::MaybeUninit::<TimsFfiFileInfo>::zeroed();
    let status = unsafe { tims_file_info(handle, info.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);
    let info = unsafe { info.assume_init() };

    let mid_rt = (info.ms2.rt_min + info.ms2.rt_max) / 2.0;
    let mid_im = (info.ms2.im_min + info.ms2.im_max) / 2.0;
    let im_lo = mid_im - 0.05;
    let im_hi = mid_im + 0.05;

    let mut count: u32 = 0;
    let mut specs: *mut TimsFfiSpectrum = ptr::null_mut();
    let status = unsafe {
        tims_get_spectra_by_rt(handle, mid_rt, 10, im_lo, im_hi, &mut count, &mut specs)
    };
    assert_status(status, TIMSFFI_OK);

    if count > 0 {
        assert!(!specs.is_null(), "specs pointer should be non-null when count > 0");
        let spec_slice = unsafe { std::slice::from_raw_parts(specs, count as usize) };
        let tolerance = 0.01;
        for (i, sp) in spec_slice.iter().enumerate() {
            assert!(
                sp.im >= im_lo - tolerance && sp.im <= im_hi + tolerance,
                "spectrum {i} IM ({}) should be within [{}, {}] (tolerance {tolerance})",
                sp.im,
                im_lo,
                im_hi
            );
        }
        unsafe { tims_free_spectrum_array(handle, specs, count) };
    }

    unsafe { tims_close(handle) };
}

#[test]
fn dia_swath_windows_populated() {
    let path = require_dia!();
    let handle = open_real(&path);

    let mut count: u32 = 0;
    let mut windows: *mut TimsFfiSwathWindow = ptr::null_mut();
    let status = unsafe { tims_get_swath_windows(handle, &mut count, &mut windows) };
    assert_status(status, TIMSFFI_OK);

    assert!(count > 0, "DIA dataset should have > 0 swath windows, got {count}");
    assert!(!windows.is_null(), "windows pointer should be non-null");

    let win_slice = unsafe { std::slice::from_raw_parts(windows, count as usize) };

    let mut min_mz = f64::INFINITY;
    let mut max_mz = f64::NEG_INFINITY;

    for (i, w) in win_slice.iter().enumerate() {
        assert!(
            w.mz_lower < w.mz_upper,
            "window {i}: mz_lower ({}) should be < mz_upper ({})",
            w.mz_lower,
            w.mz_upper
        );
        assert!(
            w.im_lower < w.im_upper,
            "window {i}: im_lower ({}) should be < im_upper ({})",
            w.im_lower,
            w.im_upper
        );
        if w.mz_lower < min_mz { min_mz = w.mz_lower; }
        if w.mz_upper > max_mz { max_mz = w.mz_upper; }
    }

    let coverage = max_mz - min_mz;
    assert!(
        coverage > 100.0,
        "swath window m/z coverage ({coverage}) should span > 100 Da (min={min_mz}, max={max_mz})"
    );

    unsafe { tims_free_swath_windows(handle, windows) };
    unsafe { tims_close(handle) };
}

// ============================================================
// Cross-cutting Tests
// ============================================================

#[test]
fn reopen_after_close() {
    let path = require_dda!();

    // First open
    let handle1 = open_real(&path);
    let count1 = unsafe { tims_num_spectra(handle1 as *const _) };
    unsafe { tims_close(handle1) };

    // Reopen
    let handle2 = open_real(&path);
    let count2 = unsafe { tims_num_spectra(handle2 as *const _) };
    unsafe { tims_close(handle2) };

    assert_eq!(
        count1, count2,
        "spectrum count should be the same after reopen: first={count1}, second={count2}"
    );
}

#[test]
fn two_handles_simultaneously() {
    let dda = require_dda!();
    let dia = require_dia!();

    let h_dda = open_real(&dda);
    let h_dia = open_real(&dia);

    let n_dda = unsafe { tims_num_spectra(h_dda as *const _) };
    let n_dia = unsafe { tims_num_spectra(h_dia as *const _) };

    assert!(n_dda > 0, "DDA handle should have > 0 spectra while DIA is also open");
    assert!(n_dia > 0, "DIA handle should have > 0 spectra while DDA is also open");

    // Query a spectrum from each to verify independence
    let mut spec_dda = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(h_dda, 0, spec_dda.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);

    let mut spec_dia = std::mem::MaybeUninit::<TimsFfiSpectrum>::zeroed();
    let status = unsafe { tims_get_spectrum(h_dia, 0, spec_dia.as_mut_ptr()) };
    assert_status(status, TIMSFFI_OK);

    unsafe { tims_close(h_dda) };
    unsafe { tims_close(h_dia) };
}
