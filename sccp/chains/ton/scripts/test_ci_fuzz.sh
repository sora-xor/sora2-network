#!/usr/bin/env bash
set -euo pipefail

echo "[sccp-ci-fuzz] check repository hygiene"
npm run check:repo-hygiene

echo "[sccp-ci-fuzz] run nightly fuzz suite"
npm run test:fuzz:nightly

echo "[sccp-ci-fuzz] OK"
