#!/bin/bash
set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/benchmark_helpers.sh"

benchmark::build_local_binary
benchmark::require_benchmark_binary "${BENCHMARK_LOCAL_BINARY}"
benchmark::run_all_runtime_targets "${BENCHMARK_LOCAL_BINARY}"
benchmark::run_overhead "${BENCHMARK_LOCAL_BINARY}"
