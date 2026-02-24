#include <iostream>
#include <chrono>
#include <string>
#include "../include/timsrust_cpp_bridge.h"

// Helper to print elapsed time in ms since a reference point
using Clock = std::chrono::high_resolution_clock;
using Ms    = std::chrono::duration<double, std::milli>;

static void elapsed(const char* label, Clock::time_point start) {
    double ms = Ms(Clock::now() - start).count();
    std::cout << "  [" << label << "] " << ms << " ms\n";
}

int main(int argc, char** argv) {
    const char* path = (argc > 1) ? argv[1] : "test.tdf"; // example path
    tims_dataset* handle = nullptr;

    auto t0 = Clock::now();
    timsffi_status status = tims_open(path, &handle);
    elapsed("tims_open", t0);
    std::cout << "tims_open status: " << (int)status << "\n";
    if (status != TIMSFFI_OK) {
        std::cerr << "Failed to open: " << path << "\n";
        // print last error from global store
        char errbuf[1024];
        timsffi_status est = tims_get_last_error(nullptr, errbuf, 1024);
        if (est == TIMSFFI_OK && errbuf[0] != 0) {
            std::cerr << "last error: " << errbuf << "\n";
        }
        return 1;
    }

    auto t1 = Clock::now();
    unsigned int n = tims_num_spectra(handle);
    elapsed("tims_num_spectra", t1);
    std::cout << "num_spectra (MS2 DIA windows): " << n << "\n";

    auto t1b = Clock::now();
    unsigned int nf = tims_num_frames(handle);
    elapsed("tims_num_frames", t1b);
    std::cout << "num_frames (raw LC frames, incl. MS1): " << nf << "\n";

    if (n > 0) {
        tims_spectrum spec;
        auto tspec_start = Clock::now();
        timsffi_status s2 = tims_get_spectrum(handle, 0, &spec);
        elapsed("tims_get_spectrum(0)", tspec_start);
        std::cout << "tims_get_spectrum status: " << (int)s2 << "\n";
        std::cout << "num_peaks: " << spec.num_peaks << "\n";
        // Try swath windows
        unsigned int wcount = 0;
        tims_swath_window* windows = nullptr;
        auto tswath_start = Clock::now();
        timsffi_status wst = tims_get_swath_windows(handle, &wcount, &windows);
        elapsed("tims_get_swath_windows", tswath_start);
        std::cout << "tims_get_swath_windows status: " << (int)wst << ", count=" << wcount << "\n";
        if (wcount > 0 && windows) {
            for (unsigned int i = 0; i < wcount; ++i) {
                auto &w = windows[i];
                std::cout << "window["<<i<<"] mz:"<<w.mz_lower<<"-"<<w.mz_upper<<" im:"<<w.im_lower<<"-"<<w.im_upper<<"\n";
            }
            tims_free_swath_windows(handle, windows);
        }

        // Try multi-spectrum fetch near rt=spec.rt_seconds
        unsigned int got = 0;
        tims_spectrum* specs = nullptr;
        auto trt_start = Clock::now();
        timsffi_status sarray = tims_get_spectra_by_rt(handle, spec.rt_seconds, 5, 0.0, 1e6, &got, &specs);
        elapsed("tims_get_spectra_by_rt", trt_start);
        std::cout << "tims_get_spectra_by_rt status: " << (int)sarray << ", got=" << got << "\n";
        if (got > 0 && specs) {
            std::cout << "first multi spec num_peaks=" << specs[0].num_peaks << "\n";
            tims_free_spectrum_array(handle, specs, got);
        }
    } else {
        std::cout << "no spectra to fetch (or minimal implementation returns 0)." << std::endl;
    }

    tims_close(handle);
    // If open failed, print last error
    unsigned int elen = 1024;
    char errbuf[1024];
    timsffi_status est = tims_get_last_error(handle, errbuf, elen);
    if (est == TIMSFFI_OK) {
        if (errbuf[0] != 0) std::cerr << "last error: " << errbuf << "\n";
    }
    return 0;
}
