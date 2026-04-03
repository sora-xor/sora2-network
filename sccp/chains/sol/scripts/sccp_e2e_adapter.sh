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
    cmd="cargo test --manifest-path program/Cargo.toml --test sccp_flow solana_program_burn_rejects_invalid_inputs_before_account_loading -- --exact"
    ;;
  mint_verify)
    cmd="cargo test --manifest-path tools/nexus-bundle-instruction-builder/Cargo.toml -- --nocapture"
    ;;
  negative_verify)
    cmd="cargo test --manifest-path tools/nexus-bundle-instruction-builder/Cargo.toml rejects -- --nocapture"
    ;;
  *)
    usage
    exit 2
    ;;
esac

if [[ "${action}" == "mint_verify" ]] && [[ -n "${SCCP_HUB_BUNDLE_JSON_PATH:-}" || -n "${SCCP_HUB_BUNDLE_SCALE_PATH:-}" || -n "${SCCP_HUB_BUNDLE_SCALE_HEX:-}" ]]; then
  set +e
  bundle_output="$(
    cd "${repo_root}"
    cargo run --quiet --manifest-path tools/nexus-bundle-instruction-builder/Cargo.toml -- 2>&1
  )"
  status=$?
  set -e

  printf '%s\n' "${bundle_output}"

  if [[ ${status} -eq 0 ]]; then
    node -e '
const [scenarioId, action, raw] = process.argv.slice(1);
const details = JSON.parse(raw);
process.stdout.write(`${JSON.stringify({
  ok: true,
  domain: "sol",
  scenario_id: scenarioId,
  action,
  assertions: ["adapter-command-succeeded", "nexus-bundle-instruction-built"],
  ...details,
})}\n`);
' "${scenario_id}" "${action}" "${bundle_output}"
    exit 0
  fi

  printf '{"ok":false,"domain":"sol","scenario_id":"%s","action":"%s","assertions":["adapter-command-failed"],"exit_code":%d}\n' "${scenario_id}" "${action}" "${status}"
  exit "${status}"
fi

set +e
(
  cd "${repo_root}"
  eval "${cmd}"
)
status=$?
set -e

if [[ ${status} -eq 0 ]]; then
  printf '{"ok":true,"domain":"sol","scenario_id":"%s","action":"%s","assertions":["adapter-command-succeeded"]}\n' "${scenario_id}" "${action}"
  exit 0
fi

printf '{"ok":false,"domain":"sol","scenario_id":"%s","action":"%s","assertions":["adapter-command-failed"],"exit_code":%d}\n' "${scenario_id}" "${action}" "${status}"
exit "${status}"
