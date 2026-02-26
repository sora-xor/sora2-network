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
domain_name="$(basename "${repo_root}" | sed 's/^sccp-//')"
scenario_id="$(node -e 'try{const p=JSON.parse(process.argv[1]);process.stdout.write(String(p.scenario_id||"unknown"));}catch{process.stdout.write("unknown");}' "${payload_json}")"

case "${action}" in
  burn)
    cmd="npx hardhat test test/sccp-router.test.js --grep 'codec matches|emits SccpBurned|burn edge cases|outbound burn'"
    ;;
  mint_verify)
    cmd="npx hardhat test test/sora-beefy-light-client-verifier.test.js --grep 'imports an MMR root and verifies a SORA-attested burn proof end-to-end|verifyBurnProof'"
    ;;
  negative_verify)
    cmd="npx hardhat test test/sccp-router.test.js --grep 'fails closed|rejects|replay|invalid|unsupported'"
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
  printf '{"ok":true,"domain":"%s","scenario_id":"%s","action":"%s","assertions":["adapter-command-succeeded"]}\n' "${domain_name}" "${scenario_id}" "${action}"
  exit 0
fi

printf '{"ok":false,"domain":"%s","scenario_id":"%s","action":"%s","assertions":["adapter-command-failed"],"exit_code":%d}\n' "${domain_name}" "${scenario_id}" "${action}" "${status}"
exit "${status}"
