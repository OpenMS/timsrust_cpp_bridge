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
└── lib/
    ├── libtimsrust_cpp_bridge.a    (timsrust_cpp_bridge.lib on Windows — no lib prefix with MSVC)
    └── cmake/
        └── timsrust_cpp_bridge/
            ├── timsrust_cpp_bridgeConfig.cmake
            └── timsrust_cpp_bridgeConfigVersion.cmake
```

The `lib/cmake/<name>/` path follows the conventional CMake installed package layout and is a default search path for `find_package()`.

Only the static library (`.a`/`.lib`) is included. The shared library (`cdylib`) output from Cargo is intentionally excluded since OpenMS will link statically.

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
2. Cache Cargo registry and target directory (`Swatinem/rust-cache`)
3. `cargo test --features with_timsrust` (validates Rust code compiles and any future tests pass)
4. `cargo build --features with_timsrust --release`
5. Smoke test: compile and link `examples/cpp_client.cpp` against the built static library, run with no arguments to verify it does not crash/segfault (non-zero exit code is expected and allowed)
6. Package: run `scripts/package.sh` to assemble tarball (header + static lib + configured CMake files). Uses `shell: bash` on all platforms (including Windows, where GitHub Actions provides Git Bash)
7. On tag builds: upload artifact via `actions/upload-artifact`

### Release Job

Runs after all matrix jobs succeed on a tag push:
- Collects all 4 platform artifacts
- Creates a GitHub Release named after the tag
- Attaches all tarballs/zips as release assets

### Versioning

Driven by git tags. The `timsrust_cpp_bridgeConfigVersion.cmake` encodes the version extracted from the tag. Uses `SameMinorVersion` compatibility (CMake 3.11+), so 0.1.1 satisfies a request for 0.1.0, but 0.2.0 does not. This respects semver conventions during the 0.x phase where minor versions may contain breaking changes.

## CMake Config Files

### `timsrust_cpp_bridgeConfig.cmake`

Stored as a template at `cmake/timsrust_cpp_bridgeConfig.cmake.in`, configured at package time with the correct library filename per platform:
- Linux/macOS: `libtimsrust_cpp_bridge.a`
- Windows (MSVC): `timsrust_cpp_bridge.lib`

```cmake
# Config file lives at <root>/lib/cmake/timsrust_cpp_bridge/timsrust_cpp_bridgeConfig.cmake
get_filename_component(_TIMSRUST_PREFIX "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)

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

Stored as a template at `cmake/timsrust_cpp_bridgeConfigVersion.cmake.in`, version injected from the git tag at package time. Generated using `write_basic_package_version_file()` from CMakePackageConfigHelpers with `SameMinorVersion` compatibility. Alternatively, the packaging script can write this file directly using the standard CMake version-file boilerplate.

## Smoke Test

Validates the build artifact is usable without requiring real `.d` datasets:

1. Compile `examples/cpp_client.cpp` against the static library and header
2. Run the binary with no arguments — verifies it does not crash or segfault. The binary already prints usage and returns exit code 1 when called with no arguments, which is correct CLI behavior. The CI step allows non-zero exit codes (e.g. `./cpp_client || true`) and only fails on signals (segfault, abort).

**Catches:** missing system link deps, ABI mismatches, platform-specific linking issues.

**Does not catch:** data reading correctness (requires real `.d` files, remains manual).

No changes needed to `examples/cpp_client.cpp` — the existing early-exit-with-usage behavior is suitable for the smoke test.

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

**Not added:**
- No top-level `CMakeLists.txt` — this is a Cargo project that produces CMake-consumable artifacts, not a CMake project itself
- No `build.rs` — cargo build is sufficient as-is
