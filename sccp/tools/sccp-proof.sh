#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHAINS_DIR="${ROOT_DIR}/sccp/chains"

print_help() {
  cat <<'EOF'
SCCP local proof helper

Usage:
  sccp-proof.sh --help
  sccp-proof.sh tron header --rpc <url> --block-number <n>
  sccp-proof.sh bsc header-rlp [args...]
  sccp-proof.sh bsc burn-proof-to-sora [args...]
  sccp-proof.sh eth extract-burn-proof-inputs [args...]
  sccp-proof.sh tron extract-burn-proof-inputs [args...]
  sccp-proof.sh ton encode-proof-cell [args...]
  sccp-proof.sh ton encode-burn-proof-to-sora [args...]
  sccp-proof.sh sol encode-burn-proof [args...]

Notes:
  - This is an in-repo SCCP wrapper. It exists so SCCP tooling no longer depends on
    sibling repositories or a separate proof-relay CLI.
  - Commands delegate to the imported SCCP chain repos under `sccp/chains/*`.
  - Users or frontends generate proof artifacts locally and submit them themselves.
EOF
}

require_dir() {
  local dir="$1"
  if [[ ! -d "${dir}" ]]; then
    echo "missing required SCCP directory: ${dir}" >&2
    exit 1
  fi
}

run_in_chain_dir() {
  local chain="$1"
  shift
  local dir="${CHAINS_DIR}/${chain}"
  require_dir "${dir}"
  (
    cd "${dir}"
    "$@"
  )
}

if [[ $# -eq 0 ]]; then
  print_help
  exit 0
fi

case "$1" in
  -h|--help|help)
    print_help
    exit 0
    ;;
esac

domain="${1:-}"
subcommand="${2:-}"
shift 2 || true

case "${domain}:${subcommand}" in
  tron:header)
    exec node "${ROOT_DIR}/sccp/tools/tron_header.mjs" "$@"
    ;;
  bsc:header-rlp)
    exec run_in_chain_dir bsc npm run build-bsc-header-rlp -- "$@"
    ;;
  bsc:burn-proof-to-sora)
    exec run_in_chain_dir bsc npm run build-burn-proof-to-sora -- "$@"
    ;;
  eth:extract-burn-proof-inputs)
    exec run_in_chain_dir eth npm run extract-burn-proof-inputs -- "$@"
    ;;
  tron:extract-burn-proof-inputs)
    exec run_in_chain_dir tron npm run extract-burn-proof-inputs -- "$@"
    ;;
  ton:encode-proof-cell)
    exec run_in_chain_dir ton npm run encode-proof-cell -- "$@"
    ;;
  ton:encode-burn-proof-to-sora)
    exec run_in_chain_dir ton npm run encode-ton-burn-proof-to-sora -- "$@"
    ;;
  sol:encode-burn-proof)
    exec run_in_chain_dir sol python3 ./scripts/encode_sora_burn_proof.py "$@"
    ;;
  *)
    echo "unsupported SCCP proof helper command: ${domain}${subcommand:+ ${subcommand}}" >&2
    print_help >&2
    exit 1
    ;;
esac
