#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

python3 -B - <<'PY'
import ast
from pathlib import Path

ast.parse(Path("scripts/deploy_mainnet.py").read_text(encoding="utf-8"), filename="scripts/deploy_mainnet.py")
PY
python3 -B scripts/deploy_mainnet.py --help >/dev/null

stub_dir="$(mktemp -d)"
work_dir="$(mktemp -d)"
out_json="$(mktemp)"
trap 'rm -rf "${stub_dir}" "${work_dir}" "${out_json}"' EXIT

cat >"${stub_dir}/solana-keygen" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" ]]; then
  echo "solana-keygen 1.14.20"
  exit 0
fi
if [[ "${1:-}" == "pubkey" ]]; then
  case "${2:-}" in
    *payer.json) echo "11111111111111111111111111111111" ;;
    *program.json) echo "BPFLoaderUpgradeab1e11111111111111111111111" ;;
    *verifier.json) echo "Vote111111111111111111111111111111111111111" ;;
    *governor.json) echo "11111111111111111111111111111111" ;;
    *) echo "unexpected keypair path: ${2:-}" >&2; exit 1 ;;
  esac
  exit 0
fi
echo "unsupported solana-keygen invocation: $*" >&2
exit 1
SH
chmod +x "${stub_dir}/solana-keygen"

cat >"${stub_dir}/solana" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" ]]; then
  echo "solana-cli 1.14.20"
  exit 0
fi
if [[ "${1:-}" == "genesis-hash" ]]; then
  echo "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
  exit 0
fi
echo "unsupported solana invocation: $*" >&2
exit 1
SH
chmod +x "${stub_dir}/solana"

printf '{}' >"${work_dir}/payer.json"
printf '{}' >"${work_dir}/program.json"
printf '{}' >"${work_dir}/verifier.json"
printf 'stub-program' >"${work_dir}/program.so"
printf 'stub-verifier' >"${work_dir}/verifier.so"

PATH="${stub_dir}:${PATH}" python3 -B scripts/deploy_mainnet.py \
  --skip-build \
  --payer-keypair "${work_dir}/payer.json" \
  --program-keypair "${work_dir}/program.json" \
  --verifier-keypair "${work_dir}/verifier.json" \
  --program-so "${work_dir}/program.so" \
  --verifier-so "${work_dir}/verifier.so" \
  --governor-pubkey "11111111111111111111111111111111" \
  --latest-beefy-block 7 \
  --current-validator-set-id 1 \
  --current-validator-set-len 4 \
  --current-validator-set-root "0x$(printf '11%.0s' {1..32})" \
  --next-validator-set-id 2 \
  --next-validator-set-len 4 \
  --next-validator-set-root "0x$(printf '22%.0s' {1..32})" \
  >"${out_json}"

python3 -B - "${out_json}" <<'PY'
import json
import sys
from pathlib import Path

raw = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
payload = "\n".join(line for line in raw if not line.startswith("+ "))
data = json.loads(payload)

assert data["bootstrap"]["commandPreview"] is not None
assert " bootstrap " in f" {data['bootstrap']['commandPreview']} "
assert "--governor-pubkey" in data["bootstrap"]["commandPreview"]
assert "--current-validator-set-root-hex" in data["bootstrap"]["commandPreview"]
assert data["bootstrap"]["validatorBootstrapInputsComplete"] is True
assert "pendingActions" not in data, data["pendingActions"]
PY
