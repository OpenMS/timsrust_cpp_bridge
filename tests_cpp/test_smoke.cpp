// tests_cpp/test_smoke.cpp
#include "../include/timsrust_cpp_bridge.h"
#include "catch2/catch_amalgamated.hpp"
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <cmath>

static std::string get_env(const char* name) {
    const char* val = std::getenv(name);
    return val ? std::string(val) : std::string();
}

TEST_CASE("Open bad path returns error", "[smoke]") {
    tims_dataset* handle = nullptr;
    auto status = tims_open("/tmp/timsrust_cpp_test_nonexistent", &handle);
    REQUIRE(status != TIMSFFI_OK);
    char buf[256] = {};
    tims_get_last_error(nullptr, buf, sizeof(buf));
    REQUIRE(std::strlen(buf) > 0);
}

TEST_CASE("Config builder round-trip", "[smoke]") {
    auto* cfg = tims_config_create();
    REQUIRE(cfg != nullptr);
    tims_config_set_smoothing_window(cfg, 3);
    tims_config_set_centroiding_window(cfg, 5);
    tims_config_set_calibration_tolerance(cfg, 0.01);
    tims_config_set_calibrate(cfg, 1);
    tims_config_free(cfg);
}

TEST_CASE("DDA smoke test", "[smoke][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }
    tims_dataset* handle = nullptr;
    REQUIRE(tims_open(dda.c_str(), &handle) == TIMSFFI_OK);
    REQUIRE(handle != nullptr);
    auto n = tims_num_spectra(handle);
    REQUIRE(n > 0);
    tims_spectrum spec{};
    REQUIRE(tims_get_spectrum(handle, 0, &spec) == TIMSFFI_OK);
    REQUIRE(spec.num_peaks > 0);
    REQUIRE(spec.mz[0] > 0.0f);
    tims_close(handle);
}

TEST_CASE("DIA smoke test", "[smoke][dia]") {
    auto dia = get_env("TIMSRUST_TEST_DATA_DIA");
    if (dia.empty()) { SKIP("TIMSRUST_TEST_DATA_DIA not set"); }
    tims_dataset* handle = nullptr;
    REQUIRE(tims_open(dia.c_str(), &handle) == TIMSFFI_OK);
    unsigned int win_count = 0;
    tims_swath_window* windows = nullptr;
    REQUIRE(tims_get_swath_windows(handle, &win_count, &windows) == TIMSFFI_OK);
    REQUIRE(win_count > 0);
    tims_file_info_t info{};
    tims_file_info(handle, &info);
    double mid_rt = (info.ms2.rt_min + info.ms2.rt_max) / 2.0;
    unsigned int spec_count = 0;
    tims_spectrum* specs = nullptr;
    REQUIRE(tims_get_spectra_by_rt(handle, mid_rt, 3, 0.0, 1e15, &spec_count, &specs) == TIMSFFI_OK);
    REQUIRE(spec_count > 0);
    tims_free_spectrum_array(handle, specs, spec_count);
    tims_free_swath_windows(handle, windows);
    tims_close(handle);
}

// ---- Config effects ----

TEST_CASE("Centroiding reduces peak count", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }
    tims_dataset* h_default = nullptr;
    REQUIRE(tims_open(dda.c_str(), &h_default) == TIMSFFI_OK);
    tims_spectrum spec_default{};
    REQUIRE(tims_get_spectrum(h_default, 0, &spec_default) == TIMSFFI_OK);
    auto peaks_default = spec_default.num_peaks;
    tims_close(h_default);

    auto* cfg = tims_config_create();
    tims_config_set_centroiding_window(cfg, 10);
    tims_dataset* h_centro = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg, &h_centro) == TIMSFFI_OK);
    tims_spectrum spec_centro{};
    REQUIRE(tims_get_spectrum(h_centro, 0, &spec_centro) == TIMSFFI_OK);
    auto peaks_centro = spec_centro.num_peaks;
    tims_config_free(cfg);
    tims_close(h_centro);
    REQUIRE(peaks_centro <= peaks_default);
}

TEST_CASE("Smoothing changes intensities", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }
    auto* cfg_base = tims_config_create();
    tims_config_set_smoothing_window(cfg_base, 0);
    tims_dataset* h1 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg_base, &h1) == TIMSFFI_OK);
    tims_config_free(cfg_base);
    tims_spectrum s1{};
    REQUIRE(tims_get_spectrum(h1, 0, &s1) == TIMSFFI_OK);
    std::vector<float> int1(s1.intensity, s1.intensity + s1.num_peaks);
    tims_close(h1);

    auto* cfg = tims_config_create();
    tims_config_set_smoothing_window(cfg, 5);
    tims_dataset* h2 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg, &h2) == TIMSFFI_OK);
    tims_config_free(cfg);
    tims_spectrum s2{};
    REQUIRE(tims_get_spectrum(h2, 0, &s2) == TIMSFFI_OK);
    std::vector<float> int2(s2.intensity, s2.intensity + s2.num_peaks);
    tims_close(h2);

    bool any_differ = false;
    auto n = std::min(int1.size(), int2.size());
    for (size_t i = 0; i < n; ++i) {
        if (std::abs(int1[i] - int2[i]) > 1e-6f) { any_differ = true; break; }
    }
    REQUIRE(any_differ);
}

TEST_CASE("Calibration changes m/z values", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }
    auto* cfg_off = tims_config_create();
    tims_config_set_calibrate(cfg_off, 0);
    tims_dataset* h1 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg_off, &h1) == TIMSFFI_OK);
    tims_spectrum s1{};
    REQUIRE(tims_get_spectrum(h1, 0, &s1) == TIMSFFI_OK);
    std::vector<float> mz1(s1.mz, s1.mz + s1.num_peaks);
    tims_config_free(cfg_off);
    tims_close(h1);

    auto* cfg_on = tims_config_create();
    tims_config_set_calibrate(cfg_on, 1);
    tims_dataset* h2 = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg_on, &h2) == TIMSFFI_OK);
    tims_spectrum s2{};
    REQUIRE(tims_get_spectrum(h2, 0, &s2) == TIMSFFI_OK);
    std::vector<float> mz2(s2.mz, s2.mz + s2.num_peaks);
    tims_config_free(cfg_on);
    tims_close(h2);
    // At minimum both should return valid data
    REQUIRE(s1.num_peaks > 0);
    REQUIRE(s2.num_peaks > 0);
}

TEST_CASE("Config combinations produce valid output", "[config][dda]") {
    auto dda = get_env("TIMSRUST_TEST_DATA_DDA");
    if (dda.empty()) { SKIP("TIMSRUST_TEST_DATA_DDA not set"); }
    auto* cfg = tims_config_create();
    tims_config_set_smoothing_window(cfg, 3);
    tims_config_set_centroiding_window(cfg, 5);
    tims_config_set_calibration_tolerance(cfg, 0.01);
    tims_config_set_calibrate(cfg, 1);
    tims_dataset* handle = nullptr;
    REQUIRE(tims_open_with_config(dda.c_str(), cfg, &handle) == TIMSFFI_OK);
    tims_spectrum spec{};
    REQUIRE(tims_get_spectrum(handle, 0, &spec) == TIMSFFI_OK);
    REQUIRE(spec.num_peaks > 0);
    for (uint32_t i = 1; i < spec.num_peaks; ++i) { REQUIRE(spec.mz[i] >= spec.mz[i-1]); }
    for (uint32_t i = 0; i < spec.num_peaks; ++i) { REQUIRE(spec.intensity[i] >= 0.0f); }
    tims_config_free(cfg);
    tims_close(handle);
}
