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
scenario_id="$(node -e 'try{const p=JSON.parse(process.argv[1]);process.stdout.write(String(p.scenario_id||"unknown"));}catch{process.stdout.write("unknown");}' "${payload_json}")"

case "${action}" in
  burn)
    cmd="node --test tests/sccp_codec.test.mjs"
    ;;
  mint_verify)
    cmd="node --test tests/sccp_jetton_flow.test.mjs --test-name-pattern 'Jetton flow: mint|submit-signature-commitment|rejects duplicate validator signer addresses'"
    ;;
  negative_verify)
    cmd="node --test tests/sccp_jetton_flow.test.mjs --test-name-pattern 'fail-closed|rejects|replay|invalid'"
    ;;
  *)
    usage
    exit 2
    ;;
esac

set +e
(
  cd "${repo_root}"
  eval "${cmd}"
)
status=$?
set -e

if [[ ${status} -eq 0 ]]; then
  printf '{"ok":true,"domain":"ton","scenario_id":"%s","action":"%s","assertions":["adapter-command-succeeded"]}\n' "${scenario_id}" "${action}"
  exit 0
fi

printf '{"ok":false,"domain":"ton","scenario_id":"%s","action":"%s","assertions":["adapter-command-failed"],"exit_code":%d}\n' "${scenario_id}" "${action}" "${status}"
exit "${status}"
