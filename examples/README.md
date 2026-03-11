# Examples.

## What the example demonstrates

- Opening a dataset with `tims_open`
- Reading counts:
  - `tims_num_spectra` (expanded MS2)
  - `tims_num_frames` (raw LC frames)
- Retrieving swath windows (`tims_get_swath_windows`)
- Running aggregate file scan (`tims_file_info`)
- Printing FileInfo-style output (similar to OpenMS FileInfo)
- Basic wall-time reporting for open and full scan operations

## Build the example

Run from repository root:

```bash
cargo build --features with_timsrust

g++ -std=c++17 examples/cpp_client.cpp \
  -Iinclude \
  -Ltarget/debug -ltimsrust_cpp_bridge \
  -Wl,-rpath,$(pwd)/target/debug \
  -o examples/cpp_client
```

## Run the example

```bash
./examples/cpp_client /path/to/your_dataset.d
```
