#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <report.json>" >&2
  exit 2
fi

report_path="$1"
if [[ ! -f "${report_path}" ]]; then
  echo "report not found: ${report_path}" >&2
  exit 2
fi

echo "# SCCP Hub Matrix Summary"
echo
echo "- Run ID: $(jq -r '.run_id // "unknown"' "${report_path}")"
echo "- Started: $(jq -r '.started_at // "unknown"' "${report_path}")"
echo "- Finished: $(jq -r '.finished_at // "unknown"' "${report_path}")"
echo "- Scenarios: $(jq -r '.summary.total // 0' "${report_path}")"
echo "- Passed: $(jq -r '.summary.passed // 0' "${report_path}")"
echo "- Failed: $(jq -r '.summary.failed // 0' "${report_path}")"

echo
if [[ "$(jq -r '.summary.failed // 0' "${report_path}")" == "0" ]]; then
  echo "All scenarios passed."
  exit 0
fi

echo "## Failed Scenarios"
jq -r '
  .scenarios[]
  | select(.ok == false)
  | . as $scenario
  | ($scenario.steps | map(select(.ok == false)) | first) as $step
  | "- " + $scenario.id + " (" + ($scenario.failure_code // "SCENARIO_FAILED") + ")"
    + " | step=" + ($step.name // "unknown")
    + " | domain=" + ($step.domain // "unknown")
    + " | action=" + ($step.action // "unknown")
    + " | log=" + ($step.log_file // "")
' "${report_path}"
