#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV_DIR="$(cd "${ROOT_DIR}/.." && pwd)"

echo "[sora2-network] cargo test -p sccp"
(cd "${ROOT_DIR}" && cargo test -p sccp)

echo "[sora2-network] cargo test -p bridge-proxy"
(cd "${ROOT_DIR}" && cargo test -p bridge-proxy)

echo "[sora2-network] cargo test -p eth-bridge"
(cd "${ROOT_DIR}" && cargo test -p eth-bridge)

echo "[sccp-eth] npm test"
(cd "${DEV_DIR}/sccp-eth" && npm test)

echo "[sccp-bsc] npm test"
(cd "${DEV_DIR}/sccp-bsc" && npm test)

echo "[sccp-tron] npm test"
(cd "${DEV_DIR}/sccp-tron" && npm test)

echo "[sccp-sol] cargo test"
(cd "${DEV_DIR}/sccp-sol" && cargo test)

echo "[sccp-sol/program] cargo test"
(cd "${DEV_DIR}/sccp-sol/program" && cargo test)

echo "[sccp-ton] npm test"
(cd "${DEV_DIR}/sccp-ton" && npm test)

echo "OK"

