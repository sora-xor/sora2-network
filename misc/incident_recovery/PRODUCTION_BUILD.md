# SORA Nexus production recovery build corridor

`production_build.py` creates isolated, nonfixture runtime artifacts in this order:
final 138 / transaction 134, hold 135 / transaction 132, then immutable bootstrap
134 / transaction 131. It does not sign, publish, change services, start nodes, or
mark a release ready. Do not run it before the actual signed mapping, ownership
proofs, recovered council, approved operator inventory and reviewed state plans
have been assembled. Public test fixtures are rejected, including the known
public test council, replacement destination and GRANDPA keys with labels removed.

The node remains 4.8.13. This corridor builds runtime Wasm only; native recorder
engines and the node have their own build and execution evidence.

The data commitment is the Blake2-256 of the canonical input-manifest JSON
(sorted keys, two-space indentation and one final newline). That manifest binds
raw input file hashes, compiler argument selection, optional mixed-work layout
and consensus numbers. It contains no generated policy or Wasm hash. Fresh
native compilation embeds this data commitment into the policy. Final138 embeds
only the policy; hold135 binds final138; bootstrap134 binds final138 and hold135.
The outer `BUILD-MANIFEST.json` is generated last and is never embedded in Wasm.

Prepare a complete public input directory and a manifest with exactly these
fields (all paths relative to its directory):

```json
{
  "schema": "sora-nexus-production-recovery-build-input-v1",
  "files": {
    "mapping.csv": {"sha256": "<64 lowercase hex>", "blake2_256": "<64 lowercase hex>", "bytes": 123}
  },
  "compose": {
    "capture": "checkpoint",
    "csv": "mapping.csv",
    "proofs": "ownership-proofs.json",
    "inventory": "storage-inventory.json",
    "economic": "economic-plan.json",
    "semantic": "semantic-plan.json",
    "council": "recovered-council.json",
    "staking-roles": "staking-roles.json"
  },
  "operatorsScale": "operators.scale",
  "derivedInventory": "derived-accounts.json",
  "mixedLayout": null,
  "consensus": {
    "previousGrandpaSetId": 0,
    "lastAbandonedSignedSlot": 0,
    "firstRecoverySlot": 1
  }
}
```

This is a schema illustration, not valid release input. Every referenced file,
including `checkpoint/capture-complete.json` and all of its state/header/metadata
pages, must appear in `files` with its actual digest and byte count. Include the
complete recursively referenced graph; only that graph is copied into an owned
read-only directory. The native capture verifier checks its completeness and
checkpoint identity. Each input file is bounded to 256 MiB, the graph to 64 GiB
and 32,768 files. No credentials or private keys belong in this directory.

Additional recognized `compose` options are `bridge-history`, `bridge-signing`,
`bridge-hashi`, `elections`, `offences`, `governance`, `referral`,
`staking-controls`, `staking-history`, `democracy` and `scheduler`. The actual
native composer requires applicable reviews for populated checkpoint storage;
omitting a necessary review fails. It verifies the supplied mapping/control
proofs, unused destinations, complete inventory, backing and guarded storage
work. An independent fresh `verify-ownership` invocation also binds the separate
derived-account inventory to the same semantic plan and checkpoint.

Optional mixed work uses a separately inventoried JSON file:

```json
{
  "schema": "sora-nexus-production-mixed-layout-v1",
  "work": [{"composed": 0}, {"envelope": "0x<typed incoming envelope>"}],
  "audits": [{"composed": 0}, {"envelope": "0x<typed incoming audit>"}],
  "terminalEnvelope": null
}
```

Every composed work item and audit must appear exactly once and preserve its
relative order. `composed` references zero-based entries from that fresh compose
result. The corridor only changes their cursor indices; it preserves the entire
guarded business payload. Additional work must be typed incoming/outgoing
operations; additional audits must be typed incoming audits. Their complete
caller-supplied envelope bytes and indices are preserved and checked by fresh
`compile-mixed-work`. A non-null terminal envelope is a committed replacement
terminal for exact replay to validate. Structural assembly does not authenticate
bridge evidence or prove the new terminal; those remain release gates. The
native emitter's policy bytes must equal the canonical assembled policy.

After the candidate is integrated, record the source inventory with:

```sh
python3.15 -B misc/incident_recovery/production_build.py inventory --output /absolute/new/source-inventory.json
```

Run `production_build.py build --help` for the required explicit input, source
inventory, native compiler and resolved legacy lock paths and SHA-256 pins.
`--output` must be a new absolute directory outside this checkout. Supply
separate existing persistent `--final-target` and `--legacy-target` directories
outside the output and checkout. Their exclusive lock cooperates with existing
fixture build lanes. The tool does not clean Cargo targets. It quarantines only
the three stale Wasm export files before each fresh build.

Cargo uses the fixed offline, locked command:

```sh
scripts/with_llvm_env.sh cargo check --offline --locked -p framenode-runtime --lib --features build-wasm-binary
```

The source contains its pinned Rust toolchain, and the build controls pin the
existing reviewed LLVM22/Wasm setup. Each build gets a fresh `FORCE_WASM_BUILD`
token. Parent and nested Cargo both run offline. Native validation and export
allow an hour; storage composition allows six hours; each Cargo build allows
24 hours. Output caps and owned-process termination remain active. Launch failure,
nonzero exit, timeout, guard failure and artifact failure retain diagnostic
receipts and never produce a successful outer manifest.

`--legacy-lock` and `--legacy-lock-sha256` are mandatory. Its package identities and all non-runtime
packages must match the preserved lock; only the existing reviewed direct
runtime `sp-trie`/`sp-externalities` additions are allowed. Optional `--final-lock`
plus its SHA permits a formatting-only final lock normalization, requiring full
parsed TOML equality. Both original and resulting hashes are recorded. Root
Cargo.lock is never changed, and a locked build that rewrites its isolated lock
fails. No current public fixture lock amendment is silently imported.

The completed bundle retains the full source exports, emitted policy, bindings,
exact helper scripts, supplied locks, compiler copy, raw process logs, PID/exit
receipts, source inventories and both Wasm exports. `source-provenance.json`
beside each `build.json` is an exact copy of the relevant source provenance.
`command` in a completed build receipt must agree with the nested actual
`cargo/execution.json`, whose digest is recorded in `processReceipt`. A build
receipt is an output, never an admission input. Earlier source files, file sets,
provenance, inputs, process receipts and artifacts remain pinned through later
builds and final manifest creation. Existing bound bootstrap exports cannot be
refreshed. Unbound development refresh inventories its actual preserved lock and
source set and remains ineligible for production builds.

A successful build still has `productionReady: false`,
`executingArtifactIdentityVerified: false`, `signingStopVerified: false` and
`operatorApprovalsVerified: false`. Supplied slot/set numbers are construction
inputs, not proof of signing cessation. Subsequent release packaging must verify
the exact artifacts, historical replay, maintenance/escape/repair behavior,
consensus boundary, bridge accounting, entitlements and operator/external inputs.

The safe future hook for `executingArtifactIdentity` is a fresh package-owned
execution of the exact copied 134/135/138 artifacts against the bound checkpoint
and deterministic recovery transcript, with Wasm execution forced and native
fallback disabled. Its in-process certificate must bind executing code hashes
and policy, every input/state witness, activation, hold/escape and restart
results to the package's bytes. It must pin and retain the process, executable,
full input graph and outputs through bundle registration, as the historical
replay hook does. A provided build receipt, archived successful replay report or
runtime-version custom section alone cannot clear this gate. This builder does
not implement or claim that execution hook.
