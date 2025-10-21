#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
CARGO_HOME_DIR="${ROOT_DIR}/.cargo"

mkdir -p "${CARGO_HOME_DIR}"

export CARGO_HOME="${CARGO_HOME_DIR}"

echo "Using CARGO_HOME=${CARGO_HOME}"
echo "Running: cargo test --locked $*"

cargo test --locked "$@"
