#!/bin/bash
set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/benchmark_helpers.sh"

REFERENCE_BINARY="/usr/local/bin/framenode"

benchmark::require_benchmark_binary "${REFERENCE_BINARY}"
benchmark::run_all_pallet_targets "${REFERENCE_BINARY}"
