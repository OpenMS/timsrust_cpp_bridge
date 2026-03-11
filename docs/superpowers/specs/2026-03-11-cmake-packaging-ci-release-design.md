# Design: CMake Packaging & CI/Release Pipeline for timsrust_cpp_bridge

## Goal

Make timsrust_cpp_bridge consumable by OpenMS (and other CMake projects) via `find_package()`, with a CI pipeline that produces pre-built release artifacts for all platforms OpenMS supports.

## Decision: Pre-built Release Tarballs + CMake Config Package

Chosen over FetchContent-based download (adds a new pattern OpenMS doesn't use) and Corrosion/build-from-source (forces Rust toolchain on all consumers). Pre-built artifacts with a CMake config package match OpenMS's existing `find_package()` dependency pattern exactly.

Static linking chosen because:
- Simplifies distribution (no runtime dependency to ship)
- C ABI is compiler-agnostic — Rust-built `.a`/`.lib` links with gcc, clang, and MSVC
- The CMake config file declares required system link deps so consumers don't need to know about them

## Release Artifact Structure

Each GitHub Release contains 4 archives, one per platform/arch:

```
timsrust_cpp_bridge-v<VERSION>-linux-x86_64.tar.gz
timsrust_cpp_bridge-v<VERSION>-linux-aarch64.tar.gz
timsrust_cpp_bridge-v<VERSION>-macos-arm64.tar.gz
timsrust_cpp_bridge-v<VERSION>-windows-x86_64.zip
```

Each archive layout:

```
timsrust_cpp_bridge/
├── include/
│   └── timsrust_cpp_bridge.h
├── lib/
│   └── libtimsrust_cpp_bridge.a    (.lib on Windows)
└── cmake/
    └── timsrust_cpp_bridge/
        ├── timsrust_cpp_bridgeConfig.cmake
        └── timsrust_cpp_bridgeConfigVersion.cmake
```

### OpenMS consumption

```cmake
find_package(timsrust_cpp_bridge REQUIRED)
target_link_libraries(OpenMS PRIVATE timsrust_cpp_bridge::timsrust_cpp_bridge)
```

OpenMS points `CMAKE_PREFIX_PATH` at the extracted archive (or installs it to a contrib-like location).

## CI Pipeline

Single workflow at `.github/workflows/release.yml`.

### Triggers

- Push/PR to `master`: build + smoke test on all platforms (validates compilation and linking)
- Tags matching `v*` (e.g. `v0.1.0`): build + package + create GitHub Release with artifacts

### Build Matrix

| Runner | Target | Rust target triple | Archive format |
|---|---|---|---|
| `ubuntu-24.04` | linux-x86_64 | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| `ubuntu-24.04-arm` | linux-aarch64 | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| `macos-14` | macos-arm64 | `aarch64-apple-darwin` | `.tar.gz` |
| `windows-2025` | windows-x86_64 | `x86_64-pc-windows-msvc` | `.zip` |

### Job Steps (per matrix entry)

1. Install Rust toolchain (`dtolnay/rust-toolchain`, stable)
2. `cargo build --features with_timsrust --release`
3. Smoke test: compile and link `examples/cpp_client.cpp` against the built static library, run with no arguments to verify clean exit
4. Package: run `scripts/package.sh` to assemble tarball (header + static lib + configured CMake files)
5. On tag builds: upload artifact via `actions/upload-artifact`

### Release Job

Runs after all matrix jobs succeed on a tag push:
- Collects all 4 platform artifacts
- Creates a GitHub Release named after the tag
- Attaches all tarballs/zips as release assets

### Versioning

Driven by git tags. The `timsrust_cpp_bridgeConfigVersion.cmake` encodes the version extracted from the tag. Uses `SameMajorVersion` compatibility (0.2.0 satisfies requests for 0.1.0; 1.0.0 does not).

## CMake Config Files

### `timsrust_cpp_bridgeConfig.cmake`

Stored as a template at `cmake/timsrust_cpp_bridgeConfig.cmake.in`, configured at package time with the correct library filename.

```cmake
get_filename_component(_TIMSRUST_PREFIX "${CMAKE_CURRENT_LIST_DIR}/../.." ABSOLUTE)

if(NOT TARGET timsrust_cpp_bridge::timsrust_cpp_bridge)
  add_library(timsrust_cpp_bridge::timsrust_cpp_bridge STATIC IMPORTED)

  set_target_properties(timsrust_cpp_bridge::timsrust_cpp_bridge PROPERTIES
    IMPORTED_LOCATION "${_TIMSRUST_PREFIX}/lib/@TIMSRUST_LIB_FILENAME@"
    INTERFACE_INCLUDE_DIRECTORIES "${_TIMSRUST_PREFIX}/include"
  )

  # Platform-specific system dependencies
  if(UNIX AND NOT APPLE)
    target_link_libraries(timsrust_cpp_bridge::timsrust_cpp_bridge
      INTERFACE pthread dl m)
  elseif(APPLE)
    target_link_libraries(timsrust_cpp_bridge::timsrust_cpp_bridge
      INTERFACE "-framework Security" "-framework SystemConfiguration" resolv)
  elseif(WIN32)
    target_link_libraries(timsrust_cpp_bridge::timsrust_cpp_bridge
      INTERFACE ws2_32 userenv bcrypt ntdll)
  endif()
endif()
```

Note: exact system link dependencies (especially macOS frameworks) will be validated empirically during CI. The smoke test catches any missing deps immediately.

### `timsrust_cpp_bridgeConfigVersion.cmake`

Stored as a template at `cmake/timsrust_cpp_bridgeConfigVersion.cmake.in`, version injected from the git tag at package time. Standard CMake version compatibility file using `SameMajorVersion`.

## Smoke Test

Validates the build artifact is usable without requiring real `.d` datasets:

1. Compile `examples/cpp_client.cpp` against the static library and header
2. Run the binary with no arguments — verifies clean exit (not crash/segfault)

**Catches:** missing system link deps, ABI mismatches, platform-specific linking issues.

**Does not catch:** data reading correctness (requires real `.d` files, remains manual).

**Required change:** `examples/cpp_client.cpp` needs a clean early exit when invoked with no arguments (currently expects a dataset path).

## Repository Structure Changes

New files:

```
.github/
  workflows/
    release.yml                          # CI + release workflow
cmake/
  timsrust_cpp_bridgeConfig.cmake.in     # CMake config template
  timsrust_cpp_bridgeConfigVersion.cmake.in  # Version config template
scripts/
  package.sh                             # Assembles release tarball
```

Modified files:

```
examples/cpp_client.cpp                  # Clean exit when no arguments
```

**Not added:**
- No top-level `CMakeLists.txt` — this is a Cargo project that produces CMake-consumable artifacts, not a CMake project itself
- No `build.rs` — cargo build is sufficient as-is
