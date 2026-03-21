#!/usr/bin/env bash
set -euo pipefail

echo "[sccp-formal-assisted] running TON formal-assisted subset"
npm run test:formal-assisted
