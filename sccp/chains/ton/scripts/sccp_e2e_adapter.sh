#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <burn|mint_verify|negative_verify> --json '<payload>'" >&2
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
scenario_id="$(node -e 'try{const p=JSON.parse(process.argv[1]);const hasScenarioId=Object.prototype.hasOwnProperty.call(p,"scenario_id");process.stdout.write(hasScenarioId && p.scenario_id !== null ? String(p.scenario_id) : "unknown");}catch{process.stdout.write("unknown");}' "${payload_json}")"

emit_result_json() {
  local ok="$1"
  local exit_code="${2:-}"
  node -e '
const [ok, scenarioId, action, exitCode] = process.argv.slice(1);
const out = {
  ok: ok === "true",
  domain: "ton",
  scenario_id: scenarioId,
  action,
  assertions: [ok === "true" ? "adapter-command-succeeded" : "adapter-command-failed"],
};
if (exitCode !== "") {
  out.exit_code = Number.parseInt(exitCode, 10);
}
process.stdout.write(`${JSON.stringify(out)}\n`);
' "${ok}" "${scenario_id}" "${action}" "${exit_code}"
}

case "${action}" in
  burn)
    cmd=(node --test tests/sccp_codec.test.mjs)
    ;;
  mint_verify)
    cmd=(
      node
      --test
      tests/sccp_jetton_flow.test.mjs
      --test-name-pattern
      "Jetton flow: mint|submit-signature-commitment|rejects duplicate validator signer addresses"
    )
    ;;
  negative_verify)
    cmd=(
      node
      --test
      tests/sccp_jetton_flow.test.mjs
      --test-name-pattern
      "fail-closed|rejects|replay|invalid"
    )
    ;;
  *)
    usage
    exit 2
    ;;
esac

set +e
(
  cd "${repo_root}"
  "${cmd[@]}"
) >&2
status=$?
set -e

if [[ ${status} -eq 0 ]]; then
  emit_result_json true
  exit 0
fi

emit_result_json false "${status}"
exit "${status}"
