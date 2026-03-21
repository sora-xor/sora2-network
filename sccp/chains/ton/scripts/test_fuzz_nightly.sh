#!/usr/bin/env bash
set -euo pipefail

FASTCHECK_RUNS=1000

echo "[sccp-fuzz-nightly] fastcheck=${FASTCHECK_RUNS}"
npm run test:fuzz:codec -- --fuzz-runs "${FASTCHECK_RUNS}"
npm run test:fuzz:proof-cell -- --fuzz-runs "${FASTCHECK_RUNS}"
