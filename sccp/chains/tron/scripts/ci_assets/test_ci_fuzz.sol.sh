#!/usr/bin/env bash
set -euo pipefail

echo "[sccp-ci-fuzz] check repository hygiene"
bash ./scripts/check_repo_hygiene.sh

echo "[sccp-ci-fuzz] run nightly fuzz suite"
bash ./scripts/test_fuzz_nightly.sh

echo "[sccp-ci-fuzz] OK"
