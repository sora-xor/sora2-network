#!/usr/bin/env bash
set -euo pipefail

chain_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
shared_dir="$(cd "${chain_root}/../evm/shared" && pwd)"

exec bash "${shared_dir}/test_ci_formal.sh" --chain-root "${chain_root}" "$@"
