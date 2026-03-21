#!/usr/bin/env bash
set -euo pipefail

PROFILE="full"
TOOLCHAIN="nightly-2025-08-06"

echo "[sccp-fuzz-nightly] profile=${PROFILE} toolchain=${TOOLCHAIN}"
./scripts/run_fuzz_bounded.sh --profile "${PROFILE}" --toolchain "${TOOLCHAIN}" --auto-install
