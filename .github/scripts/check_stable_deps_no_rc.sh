#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

MANIFESTS=()
while IFS= read -r manifest; do
  MANIFESTS+=("${manifest}")
done < <(find . -path ./target -prune -o -name Cargo.toml -print | sort)

if [[ ${#MANIFESTS[@]} -eq 0 ]]; then
  echo "[check_stable_deps_no_rc] no Cargo.toml files found" >&2
  exit 1
fi

LOCKFILE_RC_PATTERN='source = "git\+https://github\.com/(paritytech|sora-xor)/polkadot-sdk\.git\?(tag|branch)=[^"#]*-rc[0-9]*'

if python3 - "${MANIFESTS[@]}" <<'PY'
import re
import sys
from pathlib import Path

INLINE_DEP_RE = re.compile(
    r'\{(?=[^}]*git\s*=\s*"https://github\.com/(?:paritytech|sora-xor)/polkadot-sdk\.git")'
    r'(?=[^}]*(?:tag|branch)\s*=\s*"[^"]*-rc[0-9]*")[^}]*\}',
    re.S,
)
GIT_RE = re.compile(
    r'git\s*=\s*"https://github\.com/(?:paritytech|sora-xor)/polkadot-sdk\.git"'
)
RC_RE = re.compile(r'(?:tag|branch)\s*=\s*"[^"]*-rc[0-9]*"')
MAPPING_HEADER_RE = re.compile(
    r'^(?:'
    r'(?:dependencies|dev-dependencies|build-dependencies)'
    r'|workspace\.dependencies'
    r'|target\..+?\.(?:dependencies|dev-dependencies|build-dependencies)'
    r')$'
)
ENTRY_HEADER_RE = re.compile(
    r'^(?:'
    r'(?:dependencies|dev-dependencies|build-dependencies)\..+'
    r'|workspace\.dependencies\..+'
    r'|target\..+?\.(?:dependencies|dev-dependencies|build-dependencies)\..+'
    r')$'
)


def strip_comments(line: str) -> str:
    result = []
    in_basic = False
    in_literal = False
    idx = 0

    while idx < len(line):
        char = line[idx]
        if in_basic:
            result.append(char)
            if char == "\\":
                idx += 1
                if idx < len(line):
                    result.append(line[idx])
            elif char == '"':
                in_basic = False
        elif in_literal:
            result.append(char)
            if char == "'":
                in_literal = False
        else:
            if char == "#":
                break
            result.append(char)
            if char == '"':
                in_basic = True
            elif char == "'":
                in_literal = True
        idx += 1

    return "".join(result).rstrip()


def iter_blocks(text: str):
    header = None
    lines = []

    for raw_line in text.splitlines():
        line = strip_comments(raw_line)
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            if header is not None or lines:
                yield header, "\n".join(lines)
            header = stripped[1:-1].strip()
            lines = []
            continue
        lines.append(line)

    yield header, "\n".join(lines)


found = False
for manifest in sys.argv[1:]:
    text = Path(manifest).read_text(encoding="utf-8")
    for header, block in iter_blocks(text):
        if not header:
            continue
        if MAPPING_HEADER_RE.match(header):
            if INLINE_DEP_RE.search(block):
                print(f"{manifest}: [{header}] contains RC polkadot-sdk inline dependency")
                found = True
        elif ENTRY_HEADER_RE.match(header):
            if GIT_RE.search(block) and RC_RE.search(block):
                print(f"{manifest}: [{header}] contains RC polkadot-sdk dependency")
                found = True

sys.exit(0 if found else 1)
PY
then
  echo "[check_stable_deps_no_rc] RC polkadot-sdk reference detected in Cargo.toml" >&2
  exit 1
else
  status=$?
  if [[ ${status} -ne 1 ]]; then
    exit "${status}"
  fi
fi

if [[ -f Cargo.lock ]] && rg -n --pcre2 "${LOCKFILE_RC_PATTERN}" Cargo.lock; then
  echo "[check_stable_deps_no_rc] RC polkadot-sdk source detected in Cargo.lock" >&2
  exit 1
fi

echo "[check_stable_deps_no_rc] PASS"
