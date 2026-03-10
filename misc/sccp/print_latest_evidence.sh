#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACTS_DIR="${ROOT_DIR}/misc/sccp/artifacts"
HUB_ARTIFACTS_DIR="${ROOT_DIR}/misc/sccp-e2e/artifacts"

print_release_summary() {
  local summary_path="$1"
  if [[ ! -f "${summary_path}" ]]; then
    echo "  summary: missing (${summary_path})"
    return
  fi
  local status run_id started finished failed total
  status="$(jq -r '.status // "unknown"' "${summary_path}")"
  run_id="$(jq -r '.run_id // "unknown"' "${summary_path}")"
  started="$(jq -r '.started_at // "unknown"' "${summary_path}")"
  finished="$(jq -r '.finished_at // "unknown"' "${summary_path}")"
  failed="$(jq -r '.summary.failed_stages // -1' "${summary_path}")"
  total="$(jq -r '.summary.total_stages // -1' "${summary_path}")"
  echo "  run_id: ${run_id}"
  echo "  status: ${status}"
  echo "  started_at: ${started}"
  echo "  finished_at: ${finished}"
  echo "  stages: ${failed} failed / ${total} total"
}

print_hub_summary() {
  local report_path="$1"
  if [[ ! -f "${report_path}" ]]; then
    echo "  report: missing (${report_path})"
    return
  fi
  local status passed failed total run_id matrix_mode
  passed="$(jq -r '.summary.passed // -1' "${report_path}")"
  failed="$(jq -r '.summary.failed // -1' "${report_path}")"
  total="$(jq -r '.summary.total // -1' "${report_path}")"
  run_id="$(jq -r '.run_id // "unknown"' "${report_path}")"
  matrix_mode="$(jq -r '.matrix_mode // "unknown"' "${report_path}")"
  if [[ "${failed}" == "0" ]]; then
    status="passed"
  elif [[ "${failed}" =~ ^[0-9]+$ && "${failed}" -gt 0 ]]; then
    status="failed"
  else
    status="unknown"
  fi
  echo "  run_id: ${run_id}"
  echo "  status: ${status}"
  echo "  matrix_mode: ${matrix_mode}"
  echo "  scenarios: ${passed} passed / ${failed} failed / ${total} total"
}

latest_release_dir=""
latest_pr_fast_dir=""
latest_hub_dir=""
latest_hub_source=""

if [[ -d "${ARTIFACTS_DIR}" ]]; then
  latest_release_dir="$(find "${ARTIFACTS_DIR}" -maxdepth 1 -mindepth 1 -type d -name '20*' | sort -r | head -n 1 || true)"
  latest_pr_fast_dir="$(find "${ARTIFACTS_DIR}/pr-fast" -maxdepth 1 -mindepth 1 -type d -name '20*' 2>/dev/null | sort -r | head -n 1 || true)"
fi

if [[ -n "${latest_release_dir}" && -f "${latest_release_dir}/hub-matrix/report.json" ]]; then
  latest_hub_dir="${latest_release_dir}/hub-matrix"
  latest_hub_source="release-gate"
elif [[ -d "${HUB_ARTIFACTS_DIR}" ]]; then
  latest_hub_dir="$(find "${HUB_ARTIFACTS_DIR}" -maxdepth 1 -mindepth 1 -type d -name 'hub-matrix-*' | sort -r | head -n 1 || true)"
  if [[ -n "${latest_hub_dir}" ]]; then
    latest_hub_source="standalone"
  fi
fi

echo "Latest SCCP Evidence"
echo "===================="
echo

echo "[release-gate]"
if [[ -n "${latest_release_dir}" ]]; then
  echo "  dir: ${latest_release_dir}"
  print_release_summary "${latest_release_dir}/summary.json"
else
  echo "  dir: not found"
fi
echo

echo "[pr-fast]"
if [[ -n "${latest_pr_fast_dir}" ]]; then
  echo "  dir: ${latest_pr_fast_dir}"
  print_release_summary "${latest_pr_fast_dir}/summary.json"
else
  echo "  dir: not found"
fi
echo

echo "[hub-matrix]"
if [[ -n "${latest_hub_dir}" ]]; then
  echo "  dir: ${latest_hub_dir}"
  if [[ -n "${latest_hub_source}" ]]; then
    echo "  source: ${latest_hub_source}"
  fi
  print_hub_summary "${latest_hub_dir}/report.json"
else
  echo "  dir: not found"
fi
