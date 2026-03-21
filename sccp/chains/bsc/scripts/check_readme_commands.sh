#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

README_PATH="README.md"
if [[ ! -f "${README_PATH}" ]]; then
  echo "[readme-check] missing ${README_PATH}" >&2
  exit 1
fi

has_package_json=0
if [[ -f package.json ]]; then
  has_package_json=1
fi

failures=0
in_bash_block=0
line_no=0

trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

while IFS= read -r raw_line || [[ -n "$raw_line" ]]; do
  line_no=$((line_no + 1))
  line="$(trim "${raw_line}")"

  if [[ "${line}" == '```bash' ]]; then
    in_bash_block=1
    continue
  fi

  if [[ "${line}" == '```' && "${in_bash_block}" -eq 1 ]]; then
    in_bash_block=0
    continue
  fi

  if [[ "${in_bash_block}" -ne 1 ]]; then
    continue
  fi

  if [[ -z "${line}" || "${line}" == \#* || "${line}" == --* ]]; then
    continue
  fi

  if [[ "${line}" =~ ^npm[[:space:]]+run[[:space:]]+([[:alnum:]_:-]+) ]]; then
    script_name="${BASH_REMATCH[1]}"
    if [[ "${has_package_json}" -ne 1 ]]; then
      echo "[readme-check] line ${line_no}: npm script '${script_name}' referenced but package.json is missing" >&2
      failures=$((failures + 1))
      continue
    fi
    if ! jq -e --arg name "${script_name}" '.scripts[$name] != null' package.json >/dev/null 2>&1; then
      echo "[readme-check] line ${line_no}: npm script '${script_name}' not found in package.json" >&2
      failures=$((failures + 1))
    fi
    continue
  fi

  if [[ "${line}" =~ ^bash[[:space:]]+(\./scripts/[^[:space:]]+) ]]; then
    rel_path="${BASH_REMATCH[1]#./}"
    if [[ ! -f "${rel_path}" ]]; then
      echo "[readme-check] line ${line_no}: missing script '${BASH_REMATCH[1]}'" >&2
      failures=$((failures + 1))
    fi
    continue
  fi

  if [[ "${line}" =~ ^(\./scripts/[^[:space:]]+) ]]; then
    rel_path="${BASH_REMATCH[1]#./}"
    if [[ ! -f "${rel_path}" ]]; then
      echo "[readme-check] line ${line_no}: missing script '${BASH_REMATCH[1]}'" >&2
      failures=$((failures + 1))
    fi
    continue
  fi

  if [[ "${line}" =~ ^node[[:space:]]+scripts/([^[:space:]]+) ]]; then
    rel_path="scripts/${BASH_REMATCH[1]}"
    if [[ ! -f "${rel_path}" ]]; then
      echo "[readme-check] line ${line_no}: missing node script '${rel_path}'" >&2
      failures=$((failures + 1))
    fi
    continue
  fi

done <"${README_PATH}"

if [[ "${failures}" -ne 0 ]]; then
  echo "[readme-check] FAILED (${failures} issue(s))" >&2
  exit 1
fi

echo "[readme-check] OK"
