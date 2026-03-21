#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

node --check scripts/deploy_mainnet.mjs
python3 -B - <<'PY'
import ast
from pathlib import Path

ast.parse(Path("scripts/deploy_mainnet.py").read_text(encoding="utf-8"), filename="scripts/deploy_mainnet.py")
PY
python3 -B scripts/deploy_mainnet.py --help >/dev/null

mnemonic_file="$(mktemp)"
derive_usage_stderr="$(mktemp)"
deploy_missing_value_stderr="$(mktemp)"
trap 'rm -f "${mnemonic_file}" "${derive_usage_stderr}" "${deploy_missing_value_stderr}"' EXIT
printf '%s\n' 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about' >"${mnemonic_file}"

long_uri="https://example.com/$(printf 'a%.0s' {1..180})"
governor="0:0000000000000000000000000000000000000000000000000000000000000000"
sora_asset_id="0x1111111111111111111111111111111111111111111111111111111111111111"
alternate_sora_asset_id="0x2222222222222222222222222222222222222222222222222222222222222222"

if node scripts/deploy_mainnet.mjs \
  --mnemonic-file \
  --governor "${governor}" \
  --sora-asset-id "${sora_asset_id}" > /dev/null 2>"${deploy_missing_value_stderr}"; then
  echo "deploy_mainnet accepted a missing --mnemonic-file value" >&2
  exit 1
fi

if ! grep -q '^Error: Missing value for --mnemonic-file$' "${deploy_missing_value_stderr}"; then
  echo "deploy_mainnet missing-value failure did not explain the missing flag" >&2
  cat "${deploy_missing_value_stderr}" >&2
  exit 1
fi

node scripts/deploy_mainnet.mjs \
  --mnemonic-file "${mnemonic_file}" \
  --governor "${governor}" \
  --sora-asset-id "${sora_asset_id}" \
  --metadata-uri "${long_uri}" >/dev/null

python3 -B scripts/deploy_mainnet.py \
  --skip-build \
  --mnemonic-file "${mnemonic_file}" \
  --governor "${governor}" \
  --sora-asset-id "${sora_asset_id}" \
  --metadata-uri "${long_uri}" >/dev/null

deploy_master_address="$(
  node scripts/deploy_mainnet.mjs \
    --mnemonic-file "${mnemonic_file}" \
    --governor "${governor}" \
    --sora-asset-id "${sora_asset_id}" \
    --metadata-uri "${long_uri}" |
    node -e 'let s="";process.stdin.setEncoding("utf8");process.stdin.on("data",(d)=>s+=d);process.stdin.on("end",()=>process.stdout.write(JSON.parse(s).master.address));'
)"

derived_master_address="$(
  node scripts/derive_master_address.mjs \
    --governor "${governor}" \
    --sora-asset-id "${sora_asset_id}" \
    --metadata-uri "${long_uri}" |
    node -e 'let s="";process.stdin.setEncoding("utf8");process.stdin.on("data",(d)=>s+=d);process.stdin.on("end",()=>process.stdout.write(JSON.parse(s).master_address));'
)"

if [[ "${deploy_master_address}" != "${derived_master_address}" ]]; then
  echo "derive_master_address mismatch: deploy=${deploy_master_address} derived=${derived_master_address}" >&2
  exit 1
fi

primary_state_file="$(
  node scripts/deploy_mainnet.mjs \
    --mnemonic-file "${mnemonic_file}" \
    --governor "${governor}" \
    --sora-asset-id "${sora_asset_id}" |
    node -e 'let s="";process.stdin.setEncoding("utf8");process.stdin.on("data",(d)=>s+=d);process.stdin.on("end",()=>process.stdout.write(JSON.parse(s).stateFile));'
)"

alternate_state_file="$(
  node scripts/deploy_mainnet.mjs \
    --mnemonic-file "${mnemonic_file}" \
    --governor "${governor}" \
    --sora-asset-id "${alternate_sora_asset_id}" |
    node -e 'let s="";process.stdin.setEncoding("utf8");process.stdin.on("data",(d)=>s+=d);process.stdin.on("end",()=>process.stdout.write(JSON.parse(s).stateFile));'
)"

if [[ "${primary_state_file}" == "${alternate_state_file}" ]]; then
  echo "deploy_mainnet default state file collides across different sora-asset-id values: ${primary_state_file}" >&2
  exit 1
fi

if node scripts/derive_master_address.mjs \
  --governor \
  --sora-asset-id "${sora_asset_id}" > /dev/null 2>"${derive_usage_stderr}"; then
  echo "derive_master_address accepted a missing --governor value" >&2
  exit 1
fi

if ! grep -q '^Usage:$' "${derive_usage_stderr}"; then
  echo "derive_master_address missing-value failure did not print usage" >&2
  cat "${derive_usage_stderr}" >&2
  exit 1
fi
