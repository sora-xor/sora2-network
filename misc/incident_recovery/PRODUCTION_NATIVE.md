# SORA Nexus production native recorders

`production_native.py` prepares and builds native operation recorders for the
original recovery runtime 134/131 and repair runtimes 136/132 or 137/132. The
recorder is a host verification tool. It neither authorizes a recovery repair nor
replaces the Wasm engine or the runtime's incoming-transfer authentication.

The `native_recorder/original` and `native_recorder/repaired` assets are promoted reviewed sources. `native_recorder/UPSTREAM.json`
records their exact retained origins and hashes. Its historical artifact paths
are provenance only; the installed tool uses these local assets. Do not invoke
the retained low-level `prepare.py` scripts directly for production. The guarded
entry point is `production_native.py`.

## Required inputs

First use `production_build.py` to compose and build the actual production release
from its signed mapping and reviewed public evidence. Native preparation requires
the actual source export, successful `build.json`, adjacent source inventory and
provenance copy, nested launch/PID/execution receipts and logs, and the Wasm
artifact from that exact build. A public fixture cannot stand in for those inputs.

Supply the active and original policy SCALE files and their SHA-256 checksums. For
134 these must be identical. Repairs retain the original checkpoint, mappings and
recovered council. Known public fixture keys are rejected across replacement,
council and fresh GRANDPA roles. Ownership proof validation and operator approval
remain the responsibility of the existing production composition/release gates.

Pass a complete current root source inventory and its SHA-256 checksum. Source
and build output paths must be absolute, canonical and separate. Each immutable
engine/source/policy combination owns one persistent Cargo target. A repeated
build uses the same source and target with a new output directory; do not clean or
replace the warm target.

## Preparation and native build

From the repository root, invoke `python3 misc/incident_recovery/production_native.py prepare`
with these required flags (replace every value with an actual reviewed path/hash):

```
--source /absolute/production-wasm-source
--wasm-build /absolute/production-wasm-build/build.json
--wasm-build-sha256 <sha256>
--wasm /absolute/production-wasm-build/runtime-134.compact.compressed.wasm
--wasm-sha256 <sha256>
--policy /absolute/active-policy.scale --policy-sha256 <sha256>
--original-policy /absolute/original-policy.scale --original-policy-sha256 <sha256>
--root-inventory /absolute/current-source-files.json --root-inventory-sha256 <sha256>
--destination /absolute/new-native-source-lane
```

Preparation copies the export, appends only the reviewed std wrapper, adds the
recorder workspace member and applies the exact additive host lock amendment.
It verifies that every other original source byte is unchanged. The output
reports configuration and source-inventory checksums; it does not execute Cargo.

Then invoke `python3 misc/incident_recovery/production_native.py build` with:

```
--source /absolute/new-native-source-lane
--configuration-sha256 <preparation-result-configuration-sha256>
--source-inventory-sha256 <preparation-result-sourceInventory-sha256>
--output /absolute/new-native-build-attempt
```

The build reconstructs and checks the expected overlay before any compiler
process, then runs fixed offline/locked metadata, recorder tests, incoming-audit
tests, failed-incoming tests, strict Clippy and Cargo build commands. Each command
has a 24-hour bound suitable for the busy release host. Process groups are killed
on timeout or output overflow, with actual PID, exit, logs and failure receipts
retained. Source, helper, input and prior-output hashes remain guarded.

`completion.json` links the independently copied executable and engine-binding
JSON under `pins/` to the actual Cargo artifact and embedded binding. These are
the executable and binding inputs for the existing incoming replay workflow.
Compiler authenticity, actual production replay, consensus readiness, service
state and release readiness are not inferred from a successful local build.

## Repair Wasm build entry point

The canonical release builder produces 134, 135 and 138. If a forward repair is
needed, use the existing `bootstrap/prepare.py --spec-version 136` (or 137) with
the reviewed repair binding to export its legacy engine. Then invoke
`python3 misc/incident_recovery/production_native.py build-engine-wasm` with:

```
--spec-version 136
--source /absolute/exported-repair-source
--target /absolute/existing-warm-repair-wasm-target
--output /absolute/new-repair-wasm-attempt
--root-inventory /absolute/current-source-files.json --root-inventory-sha256 <sha256>
--policy /absolute/repair-policy.scale --policy-sha256 <sha256>
--original-policy /absolute/original-policy.scale --original-policy-sha256 <sha256>
--final-wasm /absolute/production-runtime-138.compact.compressed.wasm --final-wasm-sha256 <sha256>
--legacy-lock /absolute/reviewed-legacy-Cargo.lock --legacy-lock-sha256 <sha256>
```

This entry point supports only 136 and 137. It validates the compiled policies,
fresh GRANDPA keys, pinned final-138 code and export provenance, applies only the
existing reviewed legacy-lock normalization, and calls the production Wasm
builder with complete guards and retained inputs. Its actual build receipt is
`wasm/build.json`. Pass that receipt and Wasm into native preparation. This command
does not bypass council signatures or perform a repair on any chain.

Public fixture inputs, unknown source changes, unsupported locks, mismatched
policies, duplicate/foreign Cargo executable paths and changed build evidence
fail closed. Keep failed attempts as evidence. Never turn a synthetic test receipt
or an unknown compiler result into production success evidence.
