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
parachain_toolchain="${SCCP_PARACHAIN_RUSTUP_TOOLCHAIN:-nightly-2023-03-21}"

scenario_id="$(node -e 'try{const p=JSON.parse(process.argv[1]);process.stdout.write(String(p.scenario_id||"unknown"));}catch{process.stdout.write("unknown");}' "${payload_json}")"
domain_name="$(node -e '
  try {
    const p = JSON.parse(process.argv[1]);
    const src = Number(p.source_domain);
    const dst = Number(p.dest_domain);
    if (src === 6 || dst === 6) {
      process.stdout.write("sora_kusama");
    } else if (src === 7 || dst === 7) {
      process.stdout.write("sora_polkadot");
    } else {
      process.stdout.write("sora_parachain");
    }
  } catch (_) {}
' "${payload_json}")"

case "${action}" in
  burn)
    cmd="cargo test -p sccp-link burn_xor_stores_record_commits_digest_and_emits_event"
    ;;
  mint_verify)
    cmd="cargo test -p sccp-link mint_from_proof_mints_emits_event_and_rejects_replay"
    ;;
  negative_verify)
    cmd="cargo test -p sccp-link mint_from_proof_rejects_bad_source_domain && cargo test -p sccp-link mint_from_proof_fails_when_verification_fails"
    ;;
  *)
    usage
    exit 2
    ;;
esac

set +e
(
  cd "${repo_root}"
  RUSTUP_TOOLCHAIN="${parachain_toolchain}" eval "${cmd}"
)
status=$?
set -e

if [[ ${status} -eq 0 ]]; then
  printf '{"ok":true,"domain":"%s","scenario_id":"%s","action":"%s","assertions":["adapter-command-succeeded"]}\n' "${domain_name}" "${scenario_id}" "${action}"
  exit 0
fi

printf '{"ok":false,"domain":"%s","scenario_id":"%s","action":"%s","assertions":["adapter-command-failed"],"exit_code":%d}\n' "${domain_name}" "${scenario_id}" "${action}" "${status}"
exit "${status}"
