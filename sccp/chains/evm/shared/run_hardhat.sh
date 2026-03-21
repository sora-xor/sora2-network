#!/usr/bin/env bash
set -euo pipefail

require_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "missing value for ${flag}" >&2
    exit 1
  fi
}

shared_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

find_node22_bin() {
  local current_node=""
  if command -v node >/dev/null 2>&1; then
    current_node="$(command -v node)"
    local current_major
    current_major="$(node -p "process.versions.node.split('.')[0]")"
    if [[ "${current_major}" == "22" ]]; then
      printf '%s\n' "${current_node}"
      return 0
    fi
  fi

  if [[ -x "${chain_root}/scripts/select_node22_path.sh" ]]; then
    local selected_dir=""
    selected_dir="$(bash "${chain_root}/scripts/select_node22_path.sh" 2>/dev/null || true)"
    if [[ -n "${selected_dir}" && -x "${selected_dir}/node" ]]; then
      printf '%s\n' "${selected_dir}/node"
      return 0
    fi
  fi

  if command -v npx >/dev/null 2>&1; then
    local node22_bin=""
    node22_bin="$(npx -y node@22 -p "process.execPath" 2>/dev/null || true)"
    if [[ -n "${node22_bin}" && -x "${node22_bin}" ]]; then
      printf '%s\n' "${node22_bin}"
      return 0
    fi
  fi

  return 1
}

bootstrap_local_node_modules() {
  local node22_bin="$1"
  local tmp_node_path_dir
  tmp_node_path_dir="$(mktemp -d)"
  ln -s "${node22_bin}" "${tmp_node_path_dir}/node"

  local install_cmd=(npm install --no-fund --no-audit)
  if [[ -f "${chain_root}/package-lock.json" ]]; then
    install_cmd=(npm ci --no-fund --no-audit)
  fi

  echo "[sccp-hardhat] bootstrapping local npm dependencies in ${chain_root}" >&2
  set +e
  env PATH="${tmp_node_path_dir}:${PATH}" "${install_cmd[@]}"
  local status=$?
  set -e
  rm -rf "${tmp_node_path_dir}"
  return "${status}"
}

ensure_shared_node_modules_link() {
  local shared_node_modules="${shared_dir}/node_modules"
  if [[ -e "${shared_node_modules}" && ! -L "${shared_node_modules}" ]]; then
    return 0
  fi
  ln -sfn "${chain_root}/node_modules" "${shared_node_modules}"
}

chain_root=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --chain-root)
      require_value "$1" "${2-}"
      chain_root="${2}"
      shift 2
      ;;
    --)
      shift
      break
      ;;
    *)
      break
      ;;
  esac
done

if [[ -n "${chain_root}" ]]; then
  chain_root="$(cd "${chain_root}" && pwd)"
else
  chain_root="$(pwd)"
fi

cd "${chain_root}"

local_hardhat="${chain_root}/node_modules/.bin/hardhat"
node22_bin="$(find_node22_bin || true)"

if [[ -n "${node22_bin}" && -x "${local_hardhat}" ]]; then
  ensure_shared_node_modules_link
  exec "${node22_bin}" "${local_hardhat}" "$@"
fi

if [[ -n "${node22_bin}" && -f "${chain_root}/package.json" ]] && command -v npm >/dev/null 2>&1; then
  bootstrap_local_node_modules "${node22_bin}"
  if [[ -x "${local_hardhat}" ]]; then
    ensure_shared_node_modules_link
    exec "${node22_bin}" "${local_hardhat}" "$@"
  fi
fi

echo "Node 22 is required to run Hardhat. Install/use Node 22 or ensure 'npx -y node@22' is available." >&2
exit 1
