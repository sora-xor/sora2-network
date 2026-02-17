#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS_DIR="${ROOT_DIR}/scripts"

cd "${SCRIPTS_DIR}"

if [[ ! -d node_modules ]]; then
  echo "[integration] installing Node dependencies..."
  npm install
fi

node sora_dev_integration.js
