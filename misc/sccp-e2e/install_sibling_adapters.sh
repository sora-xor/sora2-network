#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATES_DIR="${ROOT_DIR}/misc/sccp-e2e/templates"
SIBLINGS_ROOT="$(cd "${ROOT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
usage: install_sibling_adapters.sh [--siblings-root <path>]

Options:
  --siblings-root <path>  Base directory containing sibling repos.
                          Defaults to one level above sora2-network.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --siblings-root)
      if [[ $# -lt 2 ]]; then
        echo "missing value for --siblings-root" >&2
        usage >&2
        exit 2
      fi
      SIBLINGS_ROOT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

copy_adapter() {
  local src="$1"
  local dst_repo="$2"
  local dst="${dst_repo}/scripts/sccp_e2e_adapter.sh"

  if [[ ! -d "${dst_repo}" ]]; then
    echo "missing sibling repo: ${dst_repo}" >&2
    return 1
  fi

  if [[ ! -d "${dst_repo}/scripts" ]]; then
    echo "missing scripts directory: ${dst_repo}/scripts" >&2
    return 1
  fi

  cp "${src}" "${dst}"
  chmod +x "${dst}"
  echo "installed adapter: ${dst}"
}

copy_adapter "${TEMPLATES_DIR}/sccp_evm_adapter.sh" "${SIBLINGS_ROOT}/sccp-eth"
copy_adapter "${TEMPLATES_DIR}/sccp_evm_adapter.sh" "${SIBLINGS_ROOT}/sccp-bsc"
copy_adapter "${TEMPLATES_DIR}/sccp_evm_adapter.sh" "${SIBLINGS_ROOT}/sccp-tron"
copy_adapter "${TEMPLATES_DIR}/sccp_sol_adapter.sh" "${SIBLINGS_ROOT}/sccp-sol"
copy_adapter "${TEMPLATES_DIR}/sccp_ton_adapter.sh" "${SIBLINGS_ROOT}/sccp-ton"
