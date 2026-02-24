#include <iostream>
#include <iomanip>
#include <chrono>
#include <string>
#include <cstring>
#include <cmath>
#include <filesystem>
#include "../include/timsrust_cpp_bridge.h"

using Clock = std::chrono::high_resolution_clock;
using Ms    = std::chrono::duration<double, std::milli>;

static double elapsed_ms(Clock::time_point start) {
    return Ms(Clock::now() - start).count();
}

// Detect whether the path is miniTDF or TDF based on presence of analysis.tdf
static std::string detect_file_type(const std::string& path) {
    namespace fs = std::filesystem;
    if (fs::exists(fs::path(path) / "analysis.tdf")) return "Bruker TDF";
    if (fs::exists(fs::path(path) / "analysis.ms2spectrum.bin")) return "Bruker miniTDF";
    return "Bruker TDF (unknown subtype)";
}

// Print a range line, or "N/A" if count == 0
static void print_range(const char* label, double lo, double hi, const char* unit = nullptr,
                        bool also_minutes = false) {
    std::cout << "  " << label << ": ";
    std::cout << std::fixed << std::setprecision(2) << lo << " .. " << hi;
    if (unit) std::cout << " " << unit;
    if (also_minutes) {
        double lo_min = lo / 60.0, hi_min = hi / 60.0;
        std::cout << " (" << std::setprecision(1) << lo_min << " .. " << hi_min << " min)";
    }
    std::cout << "\n";
}

static void print_level_stats(const tims_level_stats& s) {
    if (s.count == 0) {
        std::cout << "  (no spectra at this level)\n";
        return;
    }
    print_range("retention time", s.rt_min, s.rt_max, "sec", true);
    print_range("mass-to-charge", s.mz_min, s.mz_max);
    print_range("ion mobility",   s.im_min, s.im_max);
    print_range("intensity",      s.intensity_min, s.intensity_max);
}

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <path/to/dataset.d>\n";
        return 1;
    }
    const char* path = argv[1];

    // ---- Open ---------------------------------------------------------------
    tims_dataset* handle = nullptr;
    auto t_open = Clock::now();
    timsffi_status status = tims_open(path, &handle);
    double open_ms = elapsed_ms(t_open);

    if (status != TIMSFFI_OK) {
        std::cerr << "Failed to open dataset: " << path << "\n";
        char errbuf[1024];
        tims_get_last_error(nullptr, errbuf, sizeof(errbuf));
        if (errbuf[0]) std::cerr << "  error: " << errbuf << "\n";
        return 1;
    }

    // ---- Quick metadata (cheap) ---------------------------------------------
    unsigned int num_ms2_spectra = tims_num_spectra(handle);
    unsigned int num_frames      = tims_num_frames(handle);

    // ---- Swath windows (cheap) ----------------------------------------------
    unsigned int  wcount  = 0;
    tims_swath_window* windows = nullptr;
    tims_get_swath_windows(handle, &wcount, &windows);
    if (windows) tims_free_swath_windows(handle, windows);

    // ---- Full file statistics (parallel, slow-ish) --------------------------
    tims_file_info_t info{};
    auto t_info = Clock::now();
    timsffi_status info_status = tims_file_info(handle, &info);
    double info_ms = elapsed_ms(t_info);

    if (info_status != TIMSFFI_OK) {
        std::cerr << "tims_file_info failed\n";
        tims_close(handle);
        return 1;
    }

    // ---- Print OpenMS FileInfo-style output ---------------------------------
    std::string file_type = detect_file_type(path);

    // Combined ranges across both MS levels
    double rt_min  = std::min(info.ms1.rt_min,  info.ms2.rt_min);
    double rt_max  = std::max(info.ms1.rt_max,  info.ms2.rt_max);
    double mz_min  = std::min(info.ms1.mz_min,  info.ms2.mz_min);
    double mz_max  = std::max(info.ms1.mz_max,  info.ms2.mz_max);
    double im_min  = std::min(info.ms1.im_min,  info.ms2.im_min);
    double im_max  = std::max(info.ms1.im_max,  info.ms2.im_max);
    double int_min = std::min(info.ms1.intensity_min, info.ms2.intensity_min);
    double int_max = std::max(info.ms1.intensity_max, info.ms2.intensity_max);

    // Total spectra visible to the reader (MS1 frames not decompressed for
    // DIA-PASEF; num_frames is the raw LC frame count including MS1)
    uint32_t total_visible = info.ms1.count + info.ms2.count;
    // Determine which levels are present
    bool has_ms1 = (info.ms1.count > 0);
    bool has_ms2 = (info.ms2.count > 0);

    std::cout << "\n";
    std::cout << "-- General information --\n";
    std::cout << "File name: " << path << "\n";
    std::cout << "File type: " << file_type << "\n";
    if (has_ms1 && has_ms2) std::cout << "MS levels: 1, 2\n";
    else if (has_ms2)        std::cout << "MS levels: 2\n";
    else if (has_ms1)        std::cout << "MS levels: 1\n";
    std::cout << "Total number of peaks: " << info.total_peaks << "\n";
    std::cout << "Number of spectra: " << total_visible << "\n";
    std::cout << "  (raw LC frames: " << info.num_frames
              << ";  expanded MS2 DIA spectra: " << info.num_spectra_ms2 << ")\n";
    std::cout << "Number of DIA isolation windows: " << wcount << "\n";

    std::cout << "\nSpectrum Ranges:\n";
    if (total_visible > 0) {
        print_range("retention time", rt_min, rt_max, "sec", true);
        print_range("mass-to-charge", mz_min, mz_max);
        print_range("ion mobility",   im_min, im_max);
        print_range("intensity",       int_min, int_max);
    } else {
        std::cout << "  (no spectra)\n";
    }

    std::cout << "\nMS Level 1 Ranges:\n";
    if (has_ms1) {
        print_level_stats(info.ms1);
    } else {
        std::cout << "  (MS1 frames not exposed by the spectrum reader for this acquisition type;\n";
        std::cout << "   use num_frames=" << info.num_frames << " for the raw LC frame count)\n";
    }

    std::cout << "\nMS Level 2 Ranges:\n";
    print_level_stats(info.ms2);

    std::cout << "\nNumber of spectra per MS level:\n";
    if (has_ms1) std::cout << "  level 1: " << info.ms1.count << "\n";
    else          std::cout << "  level 1: 0  (not exposed by reader — see raw frame count)\n";
    std::cout << "  level 2: " << info.ms2.count << "\n";

    std::cout << "\n-- Timing --\n";
    std::cout << std::fixed << std::setprecision(1);
    std::cout << "  open:           " << open_ms << " ms\n";
    std::cout << "  file_info scan: " << info_ms << " ms  (wall)\n";
    std::cout << "  (timsrust internal wall_ms: " << info.wall_ms << " ms)\n";

    tims_close(handle);
    return 0;
}
