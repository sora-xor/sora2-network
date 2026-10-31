#!/usr/bin/env bash
set -euo pipefail
REHEARSAL_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
: "${REHEARSAL_NODE:=node}"
: "${CANDIDATE_SPEC_VERSION:=137}"
if [[ $# -ne 2 ]]; then
  echo "Usage: $0 /absolute/path/to/final-candidate.wasm /absolute/path/to/build-evidence.json" >&2
  exit 2
fi
exec python3 "$REHEARSAL_ROOT/run-local-rehearsals.py" "$1" \
  --build-evidence "$2" --node "$REHEARSAL_NODE" --candidate-spec "$CANDIDATE_SPEC_VERSION"
