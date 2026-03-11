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
