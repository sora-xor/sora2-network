#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEV_ROOT="$(cd "${REPO_ROOT}/.." && pwd)"
ASSETS_DIR="${SCRIPT_DIR}/ci_assets"

DEFAULT_TARGETS=(
  "sccp-tron"
  "sccp-eth"
  "sccp-bsc"
  "sccp-ton"
  "sccp-sol"
)

if [[ "$#" -gt 0 ]]; then
  TARGETS=("$@")
else
  TARGETS=("${DEFAULT_TARGETS[@]}")
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "sync_ci_assets.sh requires jq in PATH" >&2
  exit 1
fi

copy_file() {
  local src="$1"
  local dst="$2"
  mkdir -p "$(dirname "${dst}")"
  cp "${src}" "${dst}"
}

set_executable() {
  local path="$1"
  chmod 0755 "${path}"
}

sync_node_package_scripts() {
  local package_json="$1"
  local tmp
  tmp="$(mktemp)"
  jq '
    .scripts["check:repo-hygiene"] = "bash ./scripts/check_repo_hygiene.sh" |
    .scripts["test:ci-formal"] = "bash ./scripts/test_ci_formal.sh" |
    .scripts["test:ci-fuzz"] = "bash ./scripts/test_ci_fuzz.sh" |
    .scripts["test:ci-all"] = "bash ./scripts/test_ci_all.sh"
  ' "${package_json}" >"${tmp}"
  mv "${tmp}" "${package_json}"
}

sync_node_workflow_commands() {
  local repo_dir="$1"
  local formal="${repo_dir}/.github/workflows/sccp_formal_assisted.yml"
  local fuzz="${repo_dir}/.github/workflows/sccp_fuzz_nightly.yml"

  if [[ -f "${formal}" ]]; then
    perl -0pi -e 's/on:\n  pull_request:\n(?:    paths:\n(?:      - .+\n)+)?  workflow_dispatch:/on:\n  pull_request:\n  workflow_dispatch:/s; s/uses:\s*actions\/setup-python\@[^\s]+/uses: actions\/setup-python\@a26af69be951a213d495a4c3e4e4022e16d87065/g; s/run:\s*bash \.\/scripts\/test_ci_formal\.sh/run: npm run test:ci-all -- --skip-fuzz/g; s/run:\s*npm run test:formal-assisted:ci/run: npm run test:ci-all -- --skip-fuzz/g; s/run:\s*npm run test:ci-formal/run: npm run test:ci-all -- --skip-fuzz/g; s/- name: Run formal-assisted CI suite/- name: Run PR CI gate (skip fuzz)/g' "${formal}"
  fi

  if [[ -f "${fuzz}" ]]; then
    perl -0pi -e 's/run:\s*bash \.\/scripts\/test_ci_fuzz\.sh/run: npm run test:ci-fuzz/g; s/run:\s*npm run test:fuzz:nightly/run: npm run test:ci-fuzz/g' "${fuzz}"
  fi
}

sync_sol_workflow_commands() {
  local repo_dir="$1"
  local formal="${repo_dir}/.github/workflows/sccp_formal_assisted.yml"
  local fuzz="${repo_dir}/.github/workflows/sccp_fuzz_nightly.yml"

  if [[ -f "${formal}" ]]; then
    perl -0pi -e 's/on:\n  pull_request:\n(?:    paths:\n(?:      - .+\n)+)?  workflow_dispatch:/on:\n  pull_request:\n  workflow_dispatch:/s; s/uses:\s*actions\/setup-python\@[^\s]+/uses: actions\/setup-python\@a26af69be951a213d495a4c3e4e4022e16d87065/g; s/run:\s*npm run test:ci-formal/run: bash .\/scripts\/test_ci_all.sh --skip-fuzz/g; s/run:\s*bash \.\/scripts\/test_ci_formal\.sh/run: bash .\/scripts\/test_ci_all.sh --skip-fuzz/g; s/- name: Run formal-assisted CI suite/- name: Run PR CI gate (skip fuzz)/g' "${formal}"
  fi

  if [[ -f "${fuzz}" ]]; then
    perl -0pi -e 's/run:\s*npm run test:ci-fuzz/run: bash .\/scripts\/test_ci_fuzz.sh/g' "${fuzz}"
  fi
}

for repo_name in "${TARGETS[@]}"; do
  repo_dir="${DEV_ROOT}/${repo_name}"
  if [[ ! -d "${repo_dir}" ]]; then
    echo "Skipping missing repo: ${repo_name}" >&2
    continue
  fi

  echo "[sync-ci-assets] syncing ${repo_name}"

  copy_file "${ASSETS_DIR}/check_repo_hygiene.sh" "${repo_dir}/scripts/check_repo_hygiene.sh"
  copy_file "${ASSETS_DIR}/check_readme_commands.sh" "${repo_dir}/scripts/check_readme_commands.sh"
  copy_file "${ASSETS_DIR}/apply_branch_protection.sh" "${repo_dir}/scripts/apply_branch_protection.sh"
  copy_file "${ASSETS_DIR}/check_branch_protection.sh" "${repo_dir}/scripts/check_branch_protection.sh"
  copy_file "${ASSETS_DIR}/CODEOWNERS" "${repo_dir}/.github/CODEOWNERS"
  set_executable "${repo_dir}/scripts/check_repo_hygiene.sh"
  set_executable "${repo_dir}/scripts/check_readme_commands.sh"
  set_executable "${repo_dir}/scripts/apply_branch_protection.sh"
  set_executable "${repo_dir}/scripts/check_branch_protection.sh"

  copy_file "${ASSETS_DIR}/sccp_ci_lint.yml" "${repo_dir}/.github/workflows/sccp_ci_lint.yml"

  case "${repo_name}" in
    sccp-sol)
      copy_file "${ASSETS_DIR}/test_ci_formal.sol.sh" "${repo_dir}/scripts/test_ci_formal.sh"
      copy_file "${ASSETS_DIR}/test_ci_fuzz.sol.sh" "${repo_dir}/scripts/test_ci_fuzz.sh"
      copy_file "${ASSETS_DIR}/test_ci_all.sol.sh" "${repo_dir}/scripts/test_ci_all.sh"
      set_executable "${repo_dir}/scripts/test_ci_formal.sh"
      set_executable "${repo_dir}/scripts/test_ci_fuzz.sh"
      set_executable "${repo_dir}/scripts/test_ci_all.sh"
      sync_sol_workflow_commands "${repo_dir}"
      ;;
    sccp-eth)
      copy_file "${ASSETS_DIR}/test_ci_formal.eth.sh" "${repo_dir}/scripts/test_ci_formal.sh"
      copy_file "${ASSETS_DIR}/test_ci_fuzz.node.sh" "${repo_dir}/scripts/test_ci_fuzz.sh"
      copy_file "${ASSETS_DIR}/test_ci_all.node.sh" "${repo_dir}/scripts/test_ci_all.sh"
      set_executable "${repo_dir}/scripts/test_ci_formal.sh"
      set_executable "${repo_dir}/scripts/test_ci_fuzz.sh"
      set_executable "${repo_dir}/scripts/test_ci_all.sh"
      if [[ -f "${repo_dir}/package.json" ]]; then
        sync_node_package_scripts "${repo_dir}/package.json"
      fi
      sync_node_workflow_commands "${repo_dir}"
      ;;
    *)
      copy_file "${ASSETS_DIR}/test_ci_formal.node.sh" "${repo_dir}/scripts/test_ci_formal.sh"
      copy_file "${ASSETS_DIR}/test_ci_fuzz.node.sh" "${repo_dir}/scripts/test_ci_fuzz.sh"
      copy_file "${ASSETS_DIR}/test_ci_all.node.sh" "${repo_dir}/scripts/test_ci_all.sh"
      set_executable "${repo_dir}/scripts/test_ci_formal.sh"
      set_executable "${repo_dir}/scripts/test_ci_fuzz.sh"
      set_executable "${repo_dir}/scripts/test_ci_all.sh"
      if [[ -f "${repo_dir}/package.json" ]]; then
        sync_node_package_scripts "${repo_dir}/package.json"
      fi
      sync_node_workflow_commands "${repo_dir}"
      ;;
  esac

done

echo "[sync-ci-assets] done"
