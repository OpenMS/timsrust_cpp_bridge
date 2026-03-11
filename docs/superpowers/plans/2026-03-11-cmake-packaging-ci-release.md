# CMake Packaging & CI/Release Pipeline Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add CI pipeline and CMake config packaging so OpenMS can consume timsrust_cpp_bridge via `find_package()` with pre-built static library releases.

**Architecture:** GitHub Actions CI builds the Rust library on 4 platform/arch combos (linux-x86_64, linux-aarch64, macos-arm64, windows-x86_64). A packaging script assembles each build into a tarball containing the static library, C header, and CMake config files. Tag pushes trigger GitHub Releases with all 4 artifacts attached.

**Tech Stack:** Rust/Cargo, GitHub Actions, CMake (config-mode packages), Bash

**Spec:** `docs/superpowers/specs/2026-03-11-cmake-packaging-ci-release-design.md`

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `cmake/timsrust_cpp_bridgeConfig.cmake.in` | Create | CMake config template with platform system deps |
| `cmake/timsrust_cpp_bridgeConfigVersion.cmake.in` | Create | CMake version compatibility template |
| `scripts/package.sh` | Create | Assembles release tarball from cargo build output |
| `.github/workflows/release.yml` | Create | CI build + smoke test + release workflow |

No existing files are modified.

---

## Chunk 1: CMake Config Templates

### Task 1: Create CMake Config Template

**Files:**
- Create: `cmake/timsrust_cpp_bridgeConfig.cmake.in`

- [ ] **Step 1: Create the CMake config template**

```cmake
# timsrust_cpp_bridgeConfig.cmake
# Config-mode package file for timsrust_cpp_bridge
#
# Provides imported target: timsrust_cpp_bridge::timsrust_cpp_bridge
#
# This file lives at <prefix>/lib/cmake/timsrust_cpp_bridge/timsrust_cpp_bridgeConfig.cmake
# The prefix is resolved relative to this file's location.

get_filename_component(_TIMSRUST_PREFIX "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)

if(NOT TARGET timsrust_cpp_bridge::timsrust_cpp_bridge)
  add_library(timsrust_cpp_bridge::timsrust_cpp_bridge STATIC IMPORTED)

  set_target_properties(timsrust_cpp_bridge::timsrust_cpp_bridge PROPERTIES
    IMPORTED_LOCATION "${_TIMSRUST_PREFIX}/lib/@TIMSRUST_LIB_FILENAME@"
    INTERFACE_INCLUDE_DIRECTORIES "${_TIMSRUST_PREFIX}/include"
  )

  # Platform-specific system link dependencies required by the Rust static library.
  # These are validated empirically by CI smoke tests on each platform.
  if(UNIX AND NOT APPLE)
    set_property(TARGET timsrust_cpp_bridge::timsrust_cpp_bridge APPEND PROPERTY
      INTERFACE_LINK_LIBRARIES pthread dl m)
  elseif(APPLE)
    set_property(TARGET timsrust_cpp_bridge::timsrust_cpp_bridge APPEND PROPERTY
      INTERFACE_LINK_LIBRARIES "-framework Security" "-framework SystemConfiguration" resolv)
  elseif(WIN32)
    set_property(TARGET timsrust_cpp_bridge::timsrust_cpp_bridge APPEND PROPERTY
      INTERFACE_LINK_LIBRARIES ws2_32 userenv bcrypt ntdll advapi32)
  endif()
endif()

unset(_TIMSRUST_PREFIX)
```

Write this to `cmake/timsrust_cpp_bridgeConfig.cmake.in`.

- [ ] **Step 2: Verify the template placeholder**

Run: `grep '@TIMSRUST_LIB_FILENAME@' cmake/timsrust_cpp_bridgeConfig.cmake.in`
Expected: One match on the `IMPORTED_LOCATION` line.

- [ ] **Step 3: Commit**

```bash
git add cmake/timsrust_cpp_bridgeConfig.cmake.in
git commit -m "feat: add CMake config template for find_package() support"
```

### Task 2: Create CMake Version Config Template

**Files:**
- Create: `cmake/timsrust_cpp_bridgeConfigVersion.cmake.in`

- [ ] **Step 1: Create the version config template**

This is the standard CMake version compatibility boilerplate using `SameMinorVersion` semantics. The `@TIMSRUST_VERSION_MAJOR@`, `@TIMSRUST_VERSION_MINOR@`, and `@TIMSRUST_VERSION_PATCH@` placeholders are replaced by `scripts/package.sh` at package time.

```cmake
# timsrust_cpp_bridgeConfigVersion.cmake
# Auto-generated version compatibility file — SameMinorVersion semantics.

set(PACKAGE_VERSION "@TIMSRUST_VERSION_MAJOR@.@TIMSRUST_VERSION_MINOR@.@TIMSRUST_VERSION_PATCH@")

if(PACKAGE_VERSION VERSION_LESS PACKAGE_FIND_VERSION)
  set(PACKAGE_VERSION_COMPATIBLE FALSE)
else()
  # SameMinorVersion: major and minor must match exactly
  if("@TIMSRUST_VERSION_MAJOR@" EQUAL PACKAGE_FIND_VERSION_MAJOR
     AND "@TIMSRUST_VERSION_MINOR@" EQUAL PACKAGE_FIND_VERSION_MINOR)
    set(PACKAGE_VERSION_COMPATIBLE TRUE)
  else()
    set(PACKAGE_VERSION_COMPATIBLE FALSE)
  endif()

  if(PACKAGE_FIND_VERSION STREQUAL PACKAGE_VERSION)
    set(PACKAGE_VERSION_EXACT TRUE)
  endif()
endif()
```

Write this to `cmake/timsrust_cpp_bridgeConfigVersion.cmake.in`.

- [ ] **Step 2: Verify placeholders**

Run: `grep '@TIMSRUST_VERSION' cmake/timsrust_cpp_bridgeConfigVersion.cmake.in`
Expected: Multiple matches for MAJOR, MINOR, PATCH placeholders.

- [ ] **Step 3: Commit**

```bash
git add cmake/timsrust_cpp_bridgeConfigVersion.cmake.in
git commit -m "feat: add CMake version config template with SameMinorVersion semantics"
```

---

## Chunk 2: Packaging Script

### Task 3: Create package.sh

**Files:**
- Create: `scripts/package.sh`

This script runs after `cargo build --release` and assembles the release tarball. It must work under both Linux/macOS bash and Windows Git Bash (GitHub Actions `shell: bash`).

- [ ] **Step 1: Create the packaging script**

```bash
#!/usr/bin/env bash
#
# Package timsrust_cpp_bridge release artifacts.
#
# Usage: scripts/package.sh <version> <target-triple>
#   e.g.: scripts/package.sh 0.1.0 x86_64-unknown-linux-gnu
#
# Expects cargo build --release to have been run already.
# Outputs: timsrust_cpp_bridge-v<version>-<platform-label>.tar.gz (or .zip on Windows)

set -euo pipefail

VERSION="${1:?Usage: package.sh <version> <target-triple>}"
TARGET="${2:?Usage: package.sh <version> <target-triple>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Map Rust target triple to platform label and library filename
case "$TARGET" in
  x86_64-unknown-linux-gnu)
    PLATFORM="linux-x86_64"
    LIB_FILENAME="libtimsrust_cpp_bridge.a"
    ;;
  aarch64-unknown-linux-gnu)
    PLATFORM="linux-aarch64"
    LIB_FILENAME="libtimsrust_cpp_bridge.a"
    ;;
  aarch64-apple-darwin)
    PLATFORM="macos-arm64"
    LIB_FILENAME="libtimsrust_cpp_bridge.a"
    ;;
  x86_64-pc-windows-msvc)
    PLATFORM="windows-x86_64"
    LIB_FILENAME="timsrust_cpp_bridge.lib"
    ;;
  *)
    echo "Error: unknown target triple: $TARGET" >&2
    exit 1
    ;;
esac

ARCHIVE_NAME="timsrust_cpp_bridge-v${VERSION}-${PLATFORM}"
STAGING_DIR="$PROJECT_ROOT/target/package/${ARCHIVE_NAME}/timsrust_cpp_bridge"

# Clean and create staging directory
rm -rf "$PROJECT_ROOT/target/package/${ARCHIVE_NAME}"
mkdir -p "$STAGING_DIR/include"
mkdir -p "$STAGING_DIR/lib/cmake/timsrust_cpp_bridge"

# Copy header
cp "$PROJECT_ROOT/include/timsrust_cpp_bridge.h" "$STAGING_DIR/include/"

# Copy static library
cp "$PROJECT_ROOT/target/release/$LIB_FILENAME" "$STAGING_DIR/lib/"

# Configure CMake config file (replace @TIMSRUST_LIB_FILENAME@ placeholder)
sed "s|@TIMSRUST_LIB_FILENAME@|${LIB_FILENAME}|g" \
  "$PROJECT_ROOT/cmake/timsrust_cpp_bridgeConfig.cmake.in" \
  > "$STAGING_DIR/lib/cmake/timsrust_cpp_bridge/timsrust_cpp_bridgeConfig.cmake"

# Parse version components
IFS='.' read -r V_MAJOR V_MINOR V_PATCH <<< "$VERSION"

# Configure version config file
sed -e "s|@TIMSRUST_VERSION_MAJOR@|${V_MAJOR}|g" \
    -e "s|@TIMSRUST_VERSION_MINOR@|${V_MINOR}|g" \
    -e "s|@TIMSRUST_VERSION_PATCH@|${V_PATCH}|g" \
  "$PROJECT_ROOT/cmake/timsrust_cpp_bridgeConfigVersion.cmake.in" \
  > "$STAGING_DIR/lib/cmake/timsrust_cpp_bridge/timsrust_cpp_bridgeConfigVersion.cmake"

# Create archive
cd "$PROJECT_ROOT/target/package/${ARCHIVE_NAME}"
if [[ "$PLATFORM" == windows-* ]]; then
  7z a -tzip "$PROJECT_ROOT/target/package/${ARCHIVE_NAME}.zip" timsrust_cpp_bridge/
  echo "Created: target/package/${ARCHIVE_NAME}.zip"
else
  tar czf "$PROJECT_ROOT/target/package/${ARCHIVE_NAME}.tar.gz" timsrust_cpp_bridge/
  echo "Created: target/package/${ARCHIVE_NAME}.tar.gz"
fi
```

Write this to `scripts/package.sh`.

- [ ] **Step 2: Make the script executable**

Run: `chmod +x scripts/package.sh`

- [ ] **Step 3: Verify the script parses without errors**

Run: `bash -n scripts/package.sh`
Expected: No output (clean parse).

- [ ] **Step 4: Commit**

```bash
git add scripts/package.sh
git commit -m "feat: add packaging script for release artifact assembly"
```

---

## Chunk 3: GitHub Actions CI/Release Workflow

### Task 4: Create the CI/Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create the workflow file**

```yaml
name: CI & Release

on:
  push:
    branches: [master]
    tags: ['v*']
  pull_request:
    branches: [master]

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-24.04
            target: x86_64-unknown-linux-gnu
            platform: linux-x86_64
            archive_ext: tar.gz

          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
            platform: linux-aarch64
            archive_ext: tar.gz

          - os: macos-14
            target: aarch64-apple-darwin
            platform: macos-arm64
            archive_ext: tar.gz

          - os: windows-2025
            target: x86_64-pc-windows-msvc
            platform: windows-x86_64
            archive_ext: zip

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Run tests
        run: cargo test --features with_timsrust

      - name: Build release
        run: cargo build --features with_timsrust --release

      - name: Smoke test (Linux)
        if: runner.os == 'Linux'
        shell: bash
        run: |
          g++ -std=c++17 examples/cpp_client.cpp \
            -Iinclude \
            -Ltarget/release -ltimsrust_cpp_bridge \
            -lpthread -ldl -lm \
            -o target/smoke_test
          # Run with no args — expect exit code 1 (usage), fail only on signal/crash
          target/smoke_test || if [ $? -gt 128 ]; then exit 1; fi

      - name: Smoke test (macOS)
        if: runner.os == 'macOS'
        shell: bash
        run: |
          clang++ -std=c++17 examples/cpp_client.cpp \
            -Iinclude \
            -Ltarget/release -ltimsrust_cpp_bridge \
            -framework Security -framework SystemConfiguration \
            -lresolv -lpthread \
            -o target/smoke_test
          # Run with no args — expect exit code 1 (usage), fail only on signal/crash
          target/smoke_test || if [ $? -gt 128 ]; then exit 1; fi

      - name: Setup MSVC dev environment
        if: runner.os == 'Windows'
        uses: ilammy/msvc-dev-cmd@v1

      - name: Smoke test (Windows)
        if: runner.os == 'Windows'
        shell: bash
        run: |
          cl.exe /std:c++17 /EHsc /Fe:target/smoke_test.exe \
            /I include examples/cpp_client.cpp \
            target/release/timsrust_cpp_bridge.lib \
            ws2_32.lib userenv.lib bcrypt.lib ntdll.lib advapi32.lib
          target/smoke_test.exe || if [ $? -gt 128 ]; then exit 1; fi

      - name: Extract version from tag
        if: startsWith(github.ref, 'refs/tags/v')
        id: version
        shell: bash
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> "$GITHUB_OUTPUT"

      - name: Package
        if: startsWith(github.ref, 'refs/tags/v')
        shell: bash
        run: bash scripts/package.sh "${{ steps.version.outputs.version }}" "${{ matrix.target }}"

      - name: Upload artifact
        if: startsWith(github.ref, 'refs/tags/v')
        uses: actions/upload-artifact@v4
        with:
          name: timsrust_cpp_bridge-v${{ steps.version.outputs.version }}-${{ matrix.platform }}
          path: target/package/timsrust_cpp_bridge-v${{ steps.version.outputs.version }}-${{ matrix.platform }}.${{ matrix.archive_ext }}

  release:
    if: startsWith(github.ref, 'refs/tags/v')
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write

    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts/

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: artifacts/**/*
          generate_release_notes: true
```

Write this to `.github/workflows/release.yml`.

- [ ] **Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
If python3-yaml is not available, run: `python3 -c "import json, sys; print('YAML check skipped')"`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "feat: add CI/release workflow for multi-platform builds"
```

---

## Chunk 4: Local Validation

### Task 5: Validate the Full Pipeline Locally

This task validates that all the pieces fit together by doing a dry run of the packaging script against a local build.

- [ ] **Step 1: Build the library locally**

Run: `cargo build --features with_timsrust --release`
Expected: Successful build, `target/release/libtimsrust_cpp_bridge.a` exists.

- [ ] **Step 2: Run the packaging script locally**

Run: `bash scripts/package.sh 0.1.0 x86_64-unknown-linux-gnu`
Expected: Output says `Created: target/package/timsrust_cpp_bridge-v0.1.0-linux-x86_64.tar.gz`

- [ ] **Step 3: Verify tarball contents**

Run: `tar tzf target/package/timsrust_cpp_bridge-v0.1.0-linux-x86_64.tar.gz | sort`
Expected:
```
timsrust_cpp_bridge/
timsrust_cpp_bridge/include/
timsrust_cpp_bridge/include/timsrust_cpp_bridge.h
timsrust_cpp_bridge/lib/
timsrust_cpp_bridge/lib/cmake/
timsrust_cpp_bridge/lib/cmake/timsrust_cpp_bridge/
timsrust_cpp_bridge/lib/cmake/timsrust_cpp_bridge/timsrust_cpp_bridgeConfig.cmake
timsrust_cpp_bridge/lib/cmake/timsrust_cpp_bridge/timsrust_cpp_bridgeConfigVersion.cmake
timsrust_cpp_bridge/lib/libtimsrust_cpp_bridge.a
```

- [ ] **Step 4: Verify CMake config file has no remaining placeholders**

Run: `tar xzf target/package/timsrust_cpp_bridge-v0.1.0-linux-x86_64.tar.gz -C /tmp && grep '@' /tmp/timsrust_cpp_bridge/lib/cmake/timsrust_cpp_bridge/*.cmake`
Expected: No output (all placeholders replaced).

- [ ] **Step 5: Verify find_package works with a minimal CMake consumer**

Create a temporary test directory and verify CMake can find and link the package:

```bash
mkdir -p /tmp/timsrust_test && cat > /tmp/timsrust_test/CMakeLists.txt << 'CMAKEOF'
cmake_minimum_required(VERSION 3.11)
project(test_consumer C CXX)
set(CMAKE_CXX_STANDARD 17)
find_package(timsrust_cpp_bridge REQUIRED)
add_executable(test_consumer test.cpp)
target_link_libraries(test_consumer timsrust_cpp_bridge::timsrust_cpp_bridge)
CMAKEOF

cat > /tmp/timsrust_test/test.cpp << 'CPPEOF'
#include "timsrust_cpp_bridge.h"
int main() { return 0; }
CPPEOF

cmake -S /tmp/timsrust_test -B /tmp/timsrust_test/build \
  -DCMAKE_PREFIX_PATH=/tmp/timsrust_cpp_bridge
cmake --build /tmp/timsrust_test/build
/tmp/timsrust_test/build/test_consumer
```

Expected: Configures, builds, and runs without error.

- [ ] **Step 6: Clean up and commit any fixes**

If any issues were found, fix them and commit. Otherwise, no commit needed.

```bash
rm -rf /tmp/timsrust_test /tmp/timsrust_cpp_bridge target/package
```
