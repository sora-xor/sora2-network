#!/usr/bin/env bash
set -euo pipefail

FASTCHECK_RUNS=1000
FOUNDRY_RUNS=12000
ECHIDNA_TIMEOUT_SECS=1200

echo "[sccp-fuzz-nightly] fastcheck=${FASTCHECK_RUNS} foundry=${FOUNDRY_RUNS} echidna_timeout=${ECHIDNA_TIMEOUT_SECS}s"
npm run test:fuzz:fastcheck -- --runs "${FASTCHECK_RUNS}"
bash ./scripts/fuzz_foundry.sh --runs "${FOUNDRY_RUNS}"
bash ./scripts/fuzz_echidna.sh --timeout-secs "${ECHIDNA_TIMEOUT_SECS}"
