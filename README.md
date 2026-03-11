# timsrust_cpp_bridge

C ABI bridge around [timsrust](https://github.com/MannLabs/timsrust).

This project exposes a small stable C interface for reading Bruker `.d` datasets (TDF/miniTDF) through Rust, and provides a C++ example for basic usage.

## Current Status

- Working prototype with real dataset support when built with `--features with_timsrust`.
- C API is usable from C and C++ (`cdylib` and `staticlib` are produced).
- Example client in `examples/cpp_client.cpp` prints OpenMS FileInfo-like output.

[!NOTE]
> A future timsrust release is expected to include substantial refactors, so some internal Rust integration points may require updates when upgrading timsrust.

## Implemented Functionality

Header: `include/timsrust_cpp_bridge.h`

Implemented API (high level):
- Dataset lifecycle:
  - `tims_open`
  - `tims_close`
  - `tims_get_last_error`
- Counts and metadata:
  - `tims_num_spectra` (expanded MS2 spectra)
  - `tims_num_frames` (raw LC frames)
  - `tims_get_swath_windows`
  - `tims_free_swath_windows`
- Spectrum access:
  - `tims_get_spectrum`
  - `tims_get_spectra_by_rt`
  - `tims_free_spectrum_array`
- Aggregate statistics:
  - `tims_file_info` (per-level counts/ranges + total peaks + timing)

## Build

From project root:

```bash
cargo build --features with_timsrust
```

Artifacts are generated in `target/debug/` (or `target/release/` if built in release mode), including:
- `libtimsrust_cpp_bridge.so`
- static library variant

## Use From C++

Include the header and link against the built library.

### Minimal compile example

```bash
g++ -std=c++17 your_app.cpp \
  -Iinclude \
  -Ltarget/debug -ltimsrust_cpp_bridge \
  -Wl,-rpath,$(pwd)/target/debug \
  -o your_app
```

### Minimal runtime snippet

```cpp
#include <iostream>
#include "include/timsrust_cpp_bridge.h"

int main(int argc, char** argv) {
    if (argc < 2) return 1;

    tims_dataset* ds = nullptr;
    if (tims_open(argv[1], &ds) != TIMSFFI_OK) {
        char err[1024] = {0};
        tims_get_last_error(nullptr, err, sizeof(err));
        std::cerr << "open failed: " << err << "\n";
        return 1;
    }

    unsigned int ms2 = tims_num_spectra(ds);
    unsigned int frames = tims_num_frames(ds);
    std::cout << "ms2 spectra: " << ms2 << "\n";
    std::cout << "raw frames: " << frames << "\n";

    tims_file_info_t info{};
    if (tims_file_info(ds, &info) == TIMSFFI_OK) {
        std::cout << "total peaks: " << info.total_peaks << "\n";
        std::cout << "ms2 count: " << info.ms2.count << "\n";
    }

    tims_close(ds);
    return 0;
}
```

## Notes and Caveats

- For DIA-PASEF datasets, `tims_num_spectra` reflects expanded MS2 spectra, while `tims_num_frames` reflects raw LC frames (including MS1).
- `tims_file_info` performs a full scan and can take significant time on large datasets.
- Error details are available via `tims_get_last_error`.

## Example Program

See `examples/README.md` for build and run instructions for `examples/cpp_client.cpp`.
