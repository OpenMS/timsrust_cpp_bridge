// tests_cpp/test_abi.cpp
#include "../include/timsrust_cpp_bridge.h"
#include "catch2/catch_amalgamated.hpp"
#include <cstddef>
#include <cstdint>

// Enum value checks
TEST_CASE("Status enum values", "[abi]") {
    REQUIRE(TIMSFFI_OK == 0);
    REQUIRE(TIMSFFI_ERR_INVALID_UTF8 == 1);
    REQUIRE(TIMSFFI_ERR_OPEN_FAILED == 2);
    REQUIRE(TIMSFFI_ERR_INDEX_OOB == 3);
    REQUIRE(TIMSFFI_ERR_INTERNAL == 255);
}

// Struct size checks — verify they compile and have reasonable sizes
TEST_CASE("tims_spectrum sizeof", "[abi]") { REQUIRE(sizeof(tims_spectrum) > 0); }
TEST_CASE("tims_frame sizeof", "[abi]") { REQUIRE(sizeof(tims_frame) > 0); }
TEST_CASE("tims_swath_window sizeof", "[abi]") { REQUIRE(sizeof(tims_swath_window) > 0); }
TEST_CASE("tims_level_stats sizeof", "[abi]") { REQUIRE(sizeof(tims_level_stats) > 0); }
TEST_CASE("tims_file_info_t sizeof", "[abi]") { REQUIRE(sizeof(tims_file_info_t) > 0); }

// Key field offset checks
TEST_CASE("tims_spectrum field offsets", "[abi]") {
    REQUIRE(offsetof(tims_spectrum, rt_seconds) == 0);
    REQUIRE(offsetof(tims_spectrum, mz) > offsetof(tims_spectrum, num_peaks));
    REQUIRE(offsetof(tims_spectrum, intensity) > offsetof(tims_spectrum, mz));
    REQUIRE(offsetof(tims_spectrum, im) > offsetof(tims_spectrum, intensity));
}

TEST_CASE("tims_frame field offsets", "[abi]") {
    REQUIRE(offsetof(tims_frame, index) == 0);
    REQUIRE(offsetof(tims_frame, tof_indices) > offsetof(tims_frame, num_peaks));
    REQUIRE(offsetof(tims_frame, intensities) > offsetof(tims_frame, tof_indices));
    REQUIRE(offsetof(tims_frame, scan_offsets) > offsetof(tims_frame, intensities));
}

TEST_CASE("tims_swath_window field offsets", "[abi]") {
    REQUIRE(offsetof(tims_swath_window, mz_lower) == 0);
    REQUIRE(offsetof(tims_swath_window, mz_upper) == sizeof(double));
    REQUIRE(offsetof(tims_swath_window, is_ms1) > offsetof(tims_swath_window, im_upper));
}
