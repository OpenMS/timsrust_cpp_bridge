#!/usr/bin/env bash
# Download Catch2 v3.5.2 amalgamated files for the C++ test suite.
# Run from the tests_cpp/ directory (or from the repo root).
set -euo pipefail

CATCH2_VERSION="v3.5.2"
BASE_URL="https://github.com/catchorg/Catch2/releases/download/${CATCH2_VERSION}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="${SCRIPT_DIR}/catch2"

mkdir -p "${OUT_DIR}"

echo "Downloading Catch2 ${CATCH2_VERSION} amalgamated files..."
curl -fsSL -o "${OUT_DIR}/catch_amalgamated.hpp" "${BASE_URL}/catch_amalgamated.hpp"
curl -fsSL -o "${OUT_DIR}/catch_amalgamated.cpp" "${BASE_URL}/catch_amalgamated.cpp"
echo "Done. Files saved to ${OUT_DIR}/"
