# tests_cpp/Makefile
CXX      ?= g++
CXXFLAGS := -std=c++17 -Wall -Werror -I../include
LIBDIR   ?= ../target/debug
LDFLAGS  := -L$(LIBDIR) -ltimsrust_cpp_bridge -Wl,-rpath,$(realpath $(LIBDIR))

DDA ?=
DIA ?=

SRCS     := test_abi.cpp test_smoke.cpp catch2/catch_amalgamated.cpp
OBJS     := $(SRCS:.cpp=.o)
BIN      := run_tests

.PHONY: test clean

test: $(BIN)
	@echo "=== Running C++ FFI tests ==="
	TIMSRUST_TEST_DATA_DDA="$(DDA)" TIMSRUST_TEST_DATA_DIA="$(DIA)" ./$(BIN) --reporter compact

$(BIN): $(OBJS)
	$(CXX) $(CXXFLAGS) -o $@ $^ $(LDFLAGS)

%.o: %.cpp
	$(CXX) $(CXXFLAGS) -c -o $@ $<

clean:
	rm -f $(OBJS) $(BIN)
