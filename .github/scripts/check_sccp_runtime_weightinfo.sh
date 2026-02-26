#!/usr/bin/env bash
set -euo pipefail

runtime_file="runtime/src/lib.rs"

if [[ ! -f "$runtime_file" ]]; then
  echo "runtime file not found: $runtime_file" >&2
  exit 1
fi

awk '
  /impl sccp::Config for Runtime[[:space:]]*{/ {
    in_block = 1
    saw_block = 1
  }
  in_block && /type WeightInfo = sccp::weights::SubstrateWeight<Runtime>;/ {
    saw_weight = 1
  }
  in_block && /^[[:space:]]*}/ {
    in_block = 0
  }
  END {
    if (!saw_block) {
      print "missing impl sccp::Config for Runtime block in runtime/src/lib.rs" > "/dev/stderr"
      exit 1
    }
    if (!saw_weight) {
      print "SCCP runtime WeightInfo must be sccp::weights::SubstrateWeight<Runtime>" > "/dev/stderr"
      exit 1
    }
  }
' "$runtime_file"

echo "SCCP runtime WeightInfo wiring check passed."
