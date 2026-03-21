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

  node -e '
const [ok, domain, scenarioId, action, exitCode] = process.argv.slice(1);
const out = {
  ok: ok === "true",
  domain,
  scenario_id: scenarioId,
  action,
  assertions: [ok === "true" ? "adapter-command-succeeded" : "adapter-command-failed"],
};
if (exitCode !== "") {
  out.exit_code = Number(exitCode);
}
process.stdout.write(`${JSON.stringify(out)}\n`);
' "${ok}" "${domain}" "${scenario}" "${result_action}" "${exit_code}"
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

case "${action}" in
  burn)
    cmd=(
      bash ./scripts/run_hardhat.sh test test/sccp-router.test.js --grep
      "supports pause/resume via proofs and blocks burn/mint while paused|keeps burn/mint replay and recipient canonical protections"
    )
    ;;
  mint_verify)
    cmd=(
      bash ./scripts/run_hardhat.sh test test/sora-beefy-light-client-verifier.test.js --grep
      "imports finalized roots and verifies burn/add/pause/resume message proofs"
    )
    ;;
  negative_verify)
    cmd=(
      bash ./scripts/run_hardhat.sh test test/sccp-router.test.js --grep
      "fails closed|rejects|replay|invalid|unsupported"
    )
    ;;
  *)
    usage
    exit 2
    ;;
esac

output_file="$(mktemp)"
trap 'rm -f "${output_file}"' EXIT

set +e
(
  cd "${repo_root}"
  "${cmd[@]}"
) >"${output_file}" 2>&1
status=$?
set -e

cat "${output_file}"

if [[ ${status} -eq 0 ]] && grep -Eq '(^|[[:space:]])0 passing([[:space:]]|$)' "${output_file}"; then
  status=1
fi

if [[ ${status} -eq 0 ]]; then
  emit_result_json "true" "${domain_name}" "${scenario_id}" "${action}"
  exit 0
fi

emit_result_json "false" "${domain_name}" "${scenario_id}" "${action}" "${status}"
exit "${status}"
