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

# Parse Cargo manifests instead of relying on a single-line regex so multiline TOML
# dependency tables are checked as well.
if python3 - "${MANIFESTS[@]}" <<'PY'
from __future__ import annotations

import re
import sys

try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError as exc:
        print(
            "[check_stable_deps_no_rc] Python 3.11+ or the tomli package is required to inspect Cargo.toml files",
            file=sys.stderr,
        )
        raise SystemExit(2) from exc

POLKADOT_SDK_GIT_RE = re.compile(
    r"^https://github\.com/(paritytech|sora-xor)/polkadot-sdk\.git$"
)
RC_REF_RE = re.compile(r".*-rc[0-9]*$")
DEPENDENCY_SECTION_NAMES = {
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
    "patch",
    "replace",
}


def walk(node, path=()):
    if isinstance(node, dict):
        yield path, node
        for key, value in node.items():
            yield from walk(value, (*path, str(key)))
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from walk(value, (*path, str(index)))


def is_dependency_spec(path: tuple[str, ...]) -> bool:
    return any(segment in DEPENDENCY_SECTION_NAMES for segment in path)


matches: list[str] = []

for manifest_path in sys.argv[1:]:
    with open(manifest_path, "rb") as manifest_file:
        manifest = tomllib.load(manifest_file)

    for path, node in walk(manifest):
        if not is_dependency_spec(path) or not isinstance(node, dict):
            continue

        git = node.get("git")
        if not isinstance(git, str) or not POLKADOT_SDK_GIT_RE.fullmatch(git):
            continue

        for ref_kind in ("tag", "branch"):
            ref = node.get(ref_kind)
            if isinstance(ref, str) and RC_REF_RE.fullmatch(ref):
                logical_path = ".".join(path) or "<root>"
                matches.append(f"{manifest_path}:{logical_path}: {ref_kind}={ref}")

if matches:
    for match in matches:
        print(match)
    raise SystemExit(1)
PY
then
  :
else
  manifest_check_status=$?
  if [[ ${manifest_check_status} -eq 1 ]]; then
    echo "[check_stable_deps_no_rc] RC polkadot-sdk reference detected in Cargo.toml" >&2
  else
    echo "[check_stable_deps_no_rc] failed to inspect Cargo.toml files" >&2
  fi
  exit "${manifest_check_status}"
fi

if [[ -f Cargo.lock ]] && rg -n --pcre2 "${LOCKFILE_RC_PATTERN}" Cargo.lock; then
  echo "[check_stable_deps_no_rc] RC polkadot-sdk source detected in Cargo.lock" >&2
  exit 1
fi

echo "[check_stable_deps_no_rc] PASS"
