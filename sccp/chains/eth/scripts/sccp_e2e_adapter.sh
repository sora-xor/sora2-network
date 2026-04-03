#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <burn|mint_verify|negative_verify> --json '<payload>'" >&2
}

emit_result_json() {
  local ok="$1"
  local domain="$2"
  local scenario="$3"
  local result_action="$4"
  local exit_code="${5:-}"
  local details_json="${6:-}"
  local extra_assertion="${7:-}"

  node -e '
const [ok, domain, scenarioId, action, exitCode, detailsJson, extraAssertion] = process.argv.slice(1);
const out = {
  ok: ok === "true",
  domain,
  scenario_id: scenarioId,
  action,
  assertions: [ok === "true" ? "adapter-command-succeeded" : "adapter-command-failed"],
};
if (extraAssertion !== "") {
  out.assertions.push(extraAssertion);
}
if (detailsJson !== "") {
  Object.assign(out, JSON.parse(detailsJson));
}
if (exitCode !== "") {
  out.exit_code = Number(exitCode);
}
process.stdout.write(`${JSON.stringify(out)}\n`);
' "${ok}" "${domain}" "${scenario}" "${result_action}" "${exit_code}" "${details_json}" "${extra_assertion}"
}

if [[ $# -lt 3 ]]; then
  usage
  exit 2
fi

action="$1"
shift

if [[ "${1:-}" != "--json" || $# -ne 2 ]]; then
  usage
  exit 2
fi

payload_json="$2"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
domain_name="$(basename "${repo_root}" | sed 's/^sccp-//')"
scenario_id="$(node -e 'try{const p=JSON.parse(process.argv[1]);process.stdout.write(String(p.scenario_id||"unknown"));}catch{process.stdout.write("unknown");}' "${payload_json}")"

run_command_capture() {
  local output_file="$1"
  shift
  set +e
  (
    cd "${repo_root}"
    "$@"
  ) >"${output_file}" 2>&1
  local status=$?
  set -e
  cat "${output_file}"
  if [[ ${status} -eq 0 ]] && grep -Eq '(^|[[:space:]])0 passing([[:space:]]|$)' "${output_file}"; then
    status=1
  fi
  return "${status}"
}

case "${action}" in
  burn)
    cmd=(
      bash ./scripts/run_hardhat.sh test test/sccp-router.test.js --grep
      "supports pause/resume via proofs and blocks burn/mint while paused|keeps burn/mint replay and recipient canonical protections"
    )
    ;;
  mint_verify)
    cmd=(bash ./scripts/run_hardhat.sh test test/nexus-bundle-router-call.test.js)
    ;;
  negative_verify)
    cmd=(bash ./scripts/run_hardhat.sh test test/nexus-bundle-router-call.test.js --grep "rejects")
    ;;
  *)
    usage
    exit 2
    ;;
esac

if [[ "${action}" == "mint_verify" ]] && [[ -n "${SCCP_HUB_BUNDLE_JSON_PATH:-}" || -n "${SCCP_HUB_BUNDLE_SCALE_PATH:-}" || -n "${SCCP_HUB_BUNDLE_SCALE_HEX:-}" ]]; then
  output_file="$(mktemp)"
  trap 'rm -f "${output_file}"' EXIT
  set +e
  (
    cd "${repo_root}"
    node ./scripts/build_router_call_from_nexus_bundle.mjs
  ) >"${output_file}" 2>&1
  status=$?
  set -e
  cat "${output_file}"

  if [[ ${status} -eq 0 ]]; then
    details_json="$(tail -n 1 "${output_file}")"
    emit_result_json "true" "${domain_name}" "${scenario_id}" "${action}" "" "${details_json}" "nexus-bundle-router-call-built"
    exit 0
  fi

  emit_result_json "false" "${domain_name}" "${scenario_id}" "${action}" "${status}"
  exit "${status}"
fi

output_file="$(mktemp)"
trap 'rm -f "${output_file}"' EXIT

run_command_capture "${output_file}" "${cmd[@]}"
status=$?

if [[ ${status} -eq 0 ]]; then
  emit_result_json "true" "${domain_name}" "${scenario_id}" "${action}"
  exit 0
fi

emit_result_json "false" "${domain_name}" "${scenario_id}" "${action}" "${status}"
exit "${status}"
