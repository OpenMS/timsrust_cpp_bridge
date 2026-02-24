#include <iostream>
#include "../include/timsrust_cpp_bridge.h"

int main(int argc, char** argv) {
    const char* path = (argc > 1) ? argv[1] : "test.tdf"; // example path
    tims_dataset* handle = nullptr;

    int status = tims_open(path, &handle);
    std::cout << "tims_open status: " << status << "\n";
    if (status != TIMSFFI_OK) {
        std::cerr << "Failed to open: " << path << "\n";
        return 1;
    }

    unsigned int n = tims_num_spectra(handle);
    std::cout << "num_spectra: " << n << "\n";

    if (n > 0) {
        tims_spectrum spec;
        int s2 = tims_get_spectrum(handle, 0, &spec);
        std::cout << "tims_get_spectrum status: " << s2 << "\n";
        std::cout << "num_peaks: " << spec.num_peaks << "\n";
    } else {
        std::cout << "no spectra to fetch (or minimal implementation returns 0)." << std::endl;
    }

    tims_close(handle);
    return 0;
}
