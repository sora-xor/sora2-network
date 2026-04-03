#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "${REPO_ROOT}"

: "${REMOTE_RPC_URL:=https://ws.mof.sora.org}"
: "${REQUIRE_REMOTE:=1}"
export REMOTE_RPC_URL REQUIRE_REMOTE

cargo test -p framenode-runtime remote_try_runtime_upgrade_rehearsal -- --exact --nocapture "$@"
