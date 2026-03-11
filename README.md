# timsrust_cpp_bridge

C ABI bridge around [timsrust](https://github.com/MannLabs/timsrust).

This project exposes a small stable C interface for reading Bruker `.d` datasets (TDF/miniTDF) through Rust, and provides a C++ example for basic usage.

## Current Status

- Working prototype that exposes a small C ABI for reading Bruker `.d` datasets.
- The crate can be built in two modes:
  - With real reader support enabled (`--features with_timsrust`) — provides full functionality and real data access.
  - Without the feature (default) — provides lightweight stubs that allow building and exercising the C ABI surface for CI and development without the `timsrust` dependency.
- Example client in `examples/cpp_client.cpp` prints OpenMS FileInfo-like output when built against the real reader.

[!NOTE]
> A future `timsrust` release may include refactors that require updating the integration points in this crate when upgrading the dependency.

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

## Building

This crate supports two build modes. By default the crate builds without the external `timsrust` dependency and provides minimal stubs for the FFI surface. To enable real dataset reading you must enable the `with_timsrust` feature.

Build with real reader support (recommended for actual dataset access):

```bash
cargo build --features with_timsrust --release
```

Build only the FFI surface (no `timsrust` dependency; useful for CI or compilation tests):

```bash
cargo build --release
```

Artifacts are generated in `target/release/` (or `target/debug/` if not using `--release`). Expected artifacts include platform-specific shared/static libraries, for example `libtimsrust_cpp_bridge.so` on Linux.

## Use From C++

Include the header and link against the built library. When linking against a build that used `--features with_timsrust` the API will access real data; when linking against a build without the feature, the functions are present but operate as minimal stubs.

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

Additional notes:

- The `with_timsrust` feature must be enabled to access real Bruker `.d` datasets. Building without the feature will succeed but return empty/default values from reader functions — this is intentional for CI and development where `timsrust` may not be available.
- Placeholder project files included in the repository (for example `build.rs`, `cbindgen.toml`, and `src/errors.rs`) are intentionally present to keep the build surface consistent and to support downstream tooling. We recommend tracking them in git so collaborators and CI have a reproducible workspace.
- When preparing artifacts for consumption by native projects (e.g., OpenMS) consider producing a release bundle containing the shared/static library and the header file to avoid requiring consumers to use Cargo directly.

## Example Program

See `examples/README.md` for build and run instructions for `examples/cpp_client.cpp`.
