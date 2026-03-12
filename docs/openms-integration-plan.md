# OpenMS Integration Plan — timsrust_cpp_bridge

## Goal

Add native Bruker `.d` (TDF/miniTDF) reading support to OpenMS by integrating the `timsrust_cpp_bridge` static library. This replaces the need for Bruker's proprietary SDK and provides cross-platform support (Linux x86_64, Linux ARM, macOS ARM, Windows x86_64).

## Architecture

OpenMS links the pre-built `timsrust_cpp_bridge` static library (C ABI) via CMake `find_package`. A new `BrukerTdfFile` format handler wraps the C API and converts `tims_spectrum`/`tims_frame` structs into OpenMS `MSSpectrum`/`MSExperiment` objects. The dependency is optional — gated behind a `WITH_TIMSRUST` CMake option.

```
timsrust_cpp_bridge (static .a/.lib)
        |
        v  (C ABI)
BrukerTdfFile.cpp  ←  new OpenMS format handler
        |
        v
MSExperiment / MSSpectrum / Precursor  (standard OpenMS types)
```

## Prerequisites

- A tagged release of `timsrust_cpp_bridge` (e.g., `v0.1.0`) producing platform archives:
  - `timsrust_cpp_bridge-v0.1.0-linux-x86_64.tar.gz`
  - `timsrust_cpp_bridge-v0.1.0-linux-aarch64.tar.gz`
  - `timsrust_cpp_bridge-v0.1.0-macos-arm64.tar.gz`
  - `timsrust_cpp_bridge-v0.1.0-windows-x86_64.zip`
- Each archive contains: `include/timsrust_cpp_bridge.h`, `lib/libtimsrust_cpp_bridge.a` (or `.lib`), and CMake config files supporting `find_package(timsrust_cpp_bridge REQUIRED)`.

---

## Task 1: Add timsrust_cpp_bridge to OpenMS contrib / ThirdParty

### Option A: ThirdParty submodule (recommended)

Download and extract the platform archive into `OpenMS/THIRDPARTY/<platform>/timsrust_cpp_bridge/` so it sits alongside other vendored dependencies.

### Option B: System-installed package

Users run `cmake -DCMAKE_PREFIX_PATH=/path/to/timsrust_cpp_bridge ...` to point at the extracted archive.

Either way, the CMake config file shipped with the archive handles include paths, library location, and platform-specific link dependencies (pthread/dl/m on Linux, frameworks on macOS, system libs on Windows).

---

## Task 2: CMake integration

### 2.1 Add option gate

In `CMakeLists.txt` (top-level or `cmake_findExternalLibs.cmake`):

```cmake
option(WITH_TIMSRUST "Enable Bruker TDF/miniTDF support via timsrust_cpp_bridge" OFF)
```

### 2.2 Find the package

```cmake
if(WITH_TIMSRUST)
  find_package(timsrust_cpp_bridge REQUIRED)
  set(TIMSRUST_FOUND TRUE)
  add_definitions(-DWITH_TIMSRUST)
endif()
```

### 2.3 Link to OpenMS

In `src/openms/CMakeLists.txt`, add to the private dependencies:

```cmake
if(TIMSRUST_FOUND)
  list(APPEND OPENMS_DEP_PRIVATE_LIBRARIES timsrust_cpp_bridge::timsrust_cpp_bridge)
endif()
```

The `timsrust_cpp_bridge::timsrust_cpp_bridge` imported target automatically brings in the correct include directory and platform link libraries.

---

## Task 3: Register the file type

### 3.1 `FileTypes.h`

Add to the `Type` enum:

```cpp
BRUKER_TDF,  ///< Bruker TDF (.d directory, timsTOF)
```

### 3.2 `FileTypes.cpp`

Add to `type_with_annotation__`:

```cpp
{Type::BRUKER_TDF, "d", "Bruker TDF dataset (timsTOF)",
 {PROP::PROVIDES_EXPERIMENT, PROP::PROVIDES_SPECTRUM, PROP::READABLE}},
```

Note: The `.d` extension is a directory, not a file. `getTypeByFileName()` may need adjustment to check for directory existence and the presence of `analysis.tdf` inside it.

### 3.3 `FileHandler.cpp`

Add case to `loadExperiment()`:

```cpp
#ifdef WITH_TIMSRUST
case FileTypes::BRUKER_TDF:
{
  BrukerTdfFile f;
  f.setLogType(log);
  f.load(filename, exp);
  break;
}
#endif
```

---

## Task 4: Implement BrukerTdfFile

### 4.1 Header

`src/openms/include/OpenMS/FORMAT/BrukerTdfFile.h`

```cpp
#pragma once

#ifdef WITH_TIMSRUST

#include <OpenMS/KERNEL/MSExperiment.h>
#include <OpenMS/CONCEPT/ProgressLogger.h>

namespace OpenMS
{
  class OPENMS_DLLAPI BrukerTdfFile : public ProgressLogger
  {
  public:
    BrukerTdfFile() = default;

    /// Load full experiment (all spectra) from a .d directory
    void load(const String& path, MSExperiment& exp);

    /// Load only spectra near a given RT (for targeted access)
    void loadByRT(const String& path, double rt_seconds, int n_spectra,
                  double im_lower, double im_upper, MSExperiment& exp);
  };
}

#endif // WITH_TIMSRUST
```

### 4.2 Implementation

`src/openms/source/FORMAT/BrukerTdfFile.cpp`

```cpp
#ifdef WITH_TIMSRUST

#include <OpenMS/FORMAT/BrukerTdfFile.h>
#include <OpenMS/CONCEPT/Exception.h>
#include <timsrust_cpp_bridge.h>

namespace OpenMS
{

  /// RAII wrapper for tims_dataset handle
  struct TimsHandle
  {
    tims_dataset* ds = nullptr;

    ~TimsHandle()
    {
      if (ds) tims_close(ds);
    }

    void open(const String& path)
    {
      timsffi_status st = tims_open(path.c_str(), &ds);
      if (st != TIMSFFI_OK || ds == nullptr)
      {
        char err[1024]{};
        tims_get_last_error(nullptr, err, sizeof(err));
        throw Exception::FileNotReadable(
          __FILE__, __LINE__, OPENMS_PRETTY_FUNCTION, path, String(err));
      }
    }
  };

  /// Convert a tims_spectrum to an OpenMS MSSpectrum
  static MSSpectrum convertSpectrum(const tims_spectrum& ts)
  {
    MSSpectrum spec;
    spec.setRT(ts.rt_seconds);
    spec.setMSLevel(ts.ms_level);

    // Copy peak data
    spec.resize(ts.num_peaks);
    for (uint32_t j = 0; j < ts.num_peaks; ++j)
    {
      spec[j] = Peak1D(static_cast<double>(ts.mz[j]),
                        static_cast<double>(ts.intensity[j]));
    }

    // Precursor info for MS2
    if (ts.ms_level == 2)
    {
      Precursor prec;
      prec.setMZ(ts.precursor_mz);
      prec.setIntensity(ts.precursor_intensity);
      prec.setCharge(static_cast<int>(ts.charge));
      prec.setIsolationWindowLowerOffset(ts.isolation_width / 2.0);
      prec.setIsolationWindowUpperOffset(ts.isolation_width / 2.0);
      prec.setDriftTime(ts.im);
      spec.getPrecursors().push_back(prec);
    }

    // Ion mobility as drift time on the spectrum itself
    spec.setDriftTime(ts.im);

    // Native ID for traceability
    spec.setNativeID("spectrum=" + String(ts.index));

    return spec;
  }

  void BrukerTdfFile::load(const String& path, MSExperiment& exp)
  {
    TimsHandle h;
    h.open(path);

    unsigned int n = tims_num_spectra(h.ds);
    exp.clear(true);
    exp.reserve(n);

    startProgress(0, n, "Loading Bruker TDF");
    for (unsigned int i = 0; i < n; ++i)
    {
      setProgress(i);
      tims_spectrum ts{};
      if (tims_get_spectrum(h.ds, i, &ts) != TIMSFFI_OK)
      {
        continue; // skip unreadable spectra
      }
      exp.addSpectrum(convertSpectrum(ts));
    }
    endProgress();

    // Sort by RT for downstream compatibility
    exp.sortSpectra(true);
  }

  void BrukerTdfFile::loadByRT(const String& path, double rt_seconds,
                                int n_spectra, double im_lower,
                                double im_upper, MSExperiment& exp)
  {
    TimsHandle h;
    h.open(path);

    unsigned int count = 0;
    tims_spectrum* specs = nullptr;
    timsffi_status st = tims_get_spectra_by_rt(
      h.ds, rt_seconds, n_spectra, im_lower, im_upper, &count, &specs);

    if (st != TIMSFFI_OK)
    {
      char err[1024]{};
      tims_get_last_error(h.ds, err, sizeof(err));
      throw Exception::BaseException(
        __FILE__, __LINE__, OPENMS_PRETTY_FUNCTION,
        "BrukerTdfFile", String("Failed to query spectra by RT: ") + err);
    }

    exp.clear(true);
    exp.reserve(count);

    for (unsigned int i = 0; i < count; ++i)
    {
      exp.addSpectrum(convertSpectrum(specs[i]));
    }

    tims_free_spectrum_array(h.ds, specs, count);
    exp.sortSpectra(true);
  }

} // namespace OpenMS

#endif // WITH_TIMSRUST
```

### 4.3 Register source file

In `src/openms/source/FORMAT/sources.cmake`, add:

```cmake
if(WITH_TIMSRUST)
  list(APPEND FORMAT_sources BrukerTdfFile.cpp)
endif()
```

---

## Task 5: DIA-PASEF metadata

For DIA workflows, OpenMS needs SWATH/isolation window information. Add a helper that populates `SwathMap`-compatible structures from `tims_get_swath_windows`:

```cpp
/// Populate SWATH window metadata for DIA-PASEF runs
static std::vector<SwathMap> getSwathMaps(tims_dataset* ds)
{
  unsigned int count = 0;
  tims_swath_window* windows = nullptr;
  tims_get_swath_windows(ds, &count, &windows);

  std::vector<SwathMap> maps;
  maps.reserve(count);

  for (unsigned int i = 0; i < count; ++i)
  {
    SwathMap m;
    m.lower = windows[i].mz_lower;
    m.upper = windows[i].mz_upper;
    m.center = windows[i].mz_center;
    m.ms1 = (windows[i].is_ms1 != 0);
    m.imLower = windows[i].im_lower;
    m.imUpper = windows[i].im_upper;
    maps.push_back(m);
  }

  tims_free_swath_windows(ds, windows);
  return maps;
}
```

This can be exposed via `BrukerTdfFile::getSwathMaps(const String& path)` for OpenSWATH and other DIA analysis tools.

---

## Task 6: Config support (centroiding, smoothing, calibration)

Expose reader config to allow pipelines to control processing at read time:

```cpp
void BrukerTdfFile::load(const String& path, MSExperiment& exp,
                          uint32_t smoothing_window,
                          uint32_t centroiding_window,
                          bool calibrate)
{
  tims_config* cfg = tims_config_create();
  tims_config_set_smoothing_window(cfg, smoothing_window);
  tims_config_set_centroiding_window(cfg, centroiding_window);
  tims_config_set_calibrate(cfg, calibrate ? 1 : 0);

  TimsHandle h;
  // Use open_with_config instead of open
  timsffi_status st = tims_open_with_config(path.c_str(), cfg, &h.ds);
  tims_config_free(cfg);

  if (st != TIMSFFI_OK)
  {
    char err[1024]{};
    tims_get_last_error(nullptr, err, sizeof(err));
    throw Exception::FileNotReadable(
      __FILE__, __LINE__, OPENMS_PRETTY_FUNCTION, path, String(err));
  }

  // ... same loading loop as basic load() ...
}
```

Note: timsrust 0.4.2 may panic with `calibrate=on` on some datasets. The bridge catches these panics and returns `TIMSFFI_ERR_OPEN_FAILED` — handle gracefully.

---

## Task 7: Converter utilities

For frame-level workflows that work with raw TOF indices and scan numbers, expose the converters:

```cpp
/// Convert raw TOF indices from a frame to m/z values
static void convertTofToMz(tims_dataset* ds, const uint32_t* tof_indices,
                            uint32_t count, std::vector<double>& mz_out)
{
  mz_out.resize(count);
  tims_convert_tof_to_mz_array(ds, tof_indices, count, mz_out.data());
}

/// Convert scan indices to ion mobility (1/K0) values
static void convertScanToIm(tims_dataset* ds, const uint32_t* scan_indices,
                             uint32_t count, std::vector<double>& im_out)
{
  im_out.resize(count);
  tims_convert_scan_to_im_array(ds, scan_indices, count, im_out.data());
}
```

These are needed if OpenMS processes frames directly (e.g., for feature detection in the 4D RT-IM-m/z-intensity space).

---

## Task 8: Tests

### 8.1 Unit test

`src/tests/class_tests/openms/source/BrukerTdfFile_test.cpp`

```cpp
#ifdef WITH_TIMSRUST

#include <OpenMS/FORMAT/BrukerTdfFile.h>
#include <OpenMS/KERNEL/MSExperiment.h>

START_TEST(BrukerTdfFile, "$Id$")

// These tests require TIMSRUST_TEST_DATA_DDA env var pointing to a .d dataset
START_SECTION(load)
{
  const char* dda_path = std::getenv("TIMSRUST_TEST_DATA_DDA");
  if (!dda_path)
  {
    STATUS("TIMSRUST_TEST_DATA_DDA not set, skipping");
  }
  else
  {
    MSExperiment exp;
    BrukerTdfFile f;
    f.load(String(dda_path), exp);
    TEST_NOT_EQUAL(exp.size(), 0)
    TEST_EQUAL(exp[0].getMSLevel() == 1 || exp[0].getMSLevel() == 2, true)
    TEST_NOT_EQUAL(exp[0].size(), 0) // has peaks
  }
}
END_SECTION

END_TEST

#endif
```

### 8.2 Test data

Use the same datasets already stored as GitHub release artifacts:
- `DDA_HeLa_50ng_5_6min.d.zip` (345 MB) — `test-data-v1` release
- `DIA_HeLa_50ng_5_6min.d.zip` (329 MB) — same release

Set `TIMSRUST_TEST_DATA_DDA` / `TIMSRUST_TEST_DATA_DIA` in CI to run the data-dependent tests.

---

## Task 9: CI

### OpenMS CI changes

Add a job or matrix entry that:

1. Downloads the `timsrust_cpp_bridge` archive for the CI platform
2. Passes `-DWITH_TIMSRUST=ON -DCMAKE_PREFIX_PATH=<extracted_path>` to CMake
3. Runs the `BrukerTdfFile_test` with dataset env vars

The datasets can be cached using `actions/cache` (same strategy as this repo's CI).

---

## Files to create/modify in OpenMS

| Action | File | Description |
|--------|------|-------------|
| Create | `src/openms/include/OpenMS/FORMAT/BrukerTdfFile.h` | Format handler header |
| Create | `src/openms/source/FORMAT/BrukerTdfFile.cpp` | Format handler implementation |
| Create | `src/tests/class_tests/openms/source/BrukerTdfFile_test.cpp` | Unit test |
| Modify | `src/openms/include/OpenMS/FORMAT/FileTypes.h` | Add `BRUKER_TDF` enum |
| Modify | `src/openms/source/FORMAT/FileTypes.cpp` | Register type annotation |
| Modify | `src/openms/source/FORMAT/FileHandler.cpp` | Add load case |
| Modify | `src/openms/source/FORMAT/sources.cmake` | Register new source file |
| Modify | `CMakeLists.txt` or `cmake_findExternalLibs.cmake` | `WITH_TIMSRUST` option + `find_package` |

---

## API Quick Reference

| C function | Purpose | OpenMS usage |
|---|---|---|
| `tims_open` / `tims_close` | Lifecycle | RAII `TimsHandle` wrapper |
| `tims_open_with_config` | Open with processing config | Centroiding/smoothing at read time |
| `tims_num_spectra` | Expanded MS2 count (DIA-PASEF) | Reserve experiment size |
| `tims_num_frames` | Raw LC frame count | FileInfo, mzML-equivalent count |
| `tims_get_spectrum` | Single spectrum by index | Main loading loop |
| `tims_get_spectra_by_rt` | Batch query near RT + IM filter | Targeted access, `loadByRT()` |
| `tims_free_spectrum_array` | Free batch results | After `tims_get_spectra_by_rt` |
| `tims_get_frame` | Single raw frame | Frame-level 4D processing |
| `tims_get_frames_by_level` | All MS1 or MS2 frames | Bulk frame access |
| `tims_free_frame_array` | Free batch frame results | After `tims_get_frames_by_level` |
| `tims_get_swath_windows` | DIA isolation windows | OpenSWATH integration |
| `tims_free_swath_windows` | Free window array | After `tims_get_swath_windows` |
| `tims_file_info` | Aggregate statistics | FileInfo output |
| `tims_get_last_error` | Error message retrieval | Exception messages |
| `tims_convert_tof_to_mz` | TOF → m/z (scalar) | Frame-level conversion |
| `tims_convert_scan_to_im` | Scan → 1/K0 (scalar) | Frame-level conversion |
| `tims_convert_tof_to_mz_array` | TOF → m/z (batch) | Bulk frame conversion |
| `tims_convert_scan_to_im_array` | Scan → 1/K0 (batch) | Bulk frame conversion |
| `tims_config_create` / `_free` | Config lifecycle | Processing params |
| `tims_config_set_*` | Config setters | Centroiding, smoothing, calibration |

---

## Memory Ownership Summary

| API | Ownership | Lifetime | Free with |
|---|---|---|---|
| `tims_get_spectrum` | Handle-owned buffers | Until next call on same handle | Don't free — just copy |
| `tims_get_spectra_by_rt` | Caller-owned (malloc'd) | Until freed | `tims_free_spectrum_array` |
| `tims_get_frame` | Handle-owned buffers | Until next `tims_get_frame` on same handle | Don't free — just copy |
| `tims_get_frames_by_level` | Caller-owned (malloc'd) | Until freed | `tims_free_frame_array` |
| `tims_get_swath_windows` | Caller-owned (malloc'd) | Until freed | `tims_free_swath_windows` |
