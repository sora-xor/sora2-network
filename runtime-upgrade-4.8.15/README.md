# SORA Nexus 4.8.15 upgrade candidate

Runtime spec 141, transaction version 134. The package contains the mainnet Wasm and
unsigned governance calls for the upgrade from the finalized runtime.

| Artifact | Verified value |
| --- | --- |
| Mainnet Wasm | 2,151,777 bytes, SHA-256 `41fbec1aee1a828e4f86eab4b44bea174e5da1406c2b5d9d83bfecdeb79449bc` |
| `system.setCode` preimage | 2,151,783 bytes, BLAKE2-256 `0x916b010bffddfb8c414f1efda3f7efb84b3fd05c1f5997680fb268e77ae9fcdf` |
| Governance metadata | Finalized block 28,304,118, SHA-256 `0d8aaf8c1f6974e2256a0b7bb71c16d1fb502c1d1327e6ca46c44e0f554af3c2` |

The direct rehearsal passed 19 checks. At mainnet block
28,296,247, the script verified genesis, the canonical finalized block,
the deployed spec-138 Wasm and the recovery policy. The fork installed
spec 141, preserved native issuance and Root’s latch, and accepted a fee-free guardian halt.

A second rehearsal passed 19 checks on a fork of the same block. It installed the
published spec-140 Wasm, assigned a replacement guardian, and recorded halt 1. Spec 141
preserved the guardian list, halt record, and worker signal byte for byte. The same
guardian then issued halt 2. This local transition is recorded in
`via-140-rehearsal.json`. Only the direct rehearsal qualifies governance calls.

## Bridge peers

Build the 4.8.15 node from the source commit below and upgrade HASHI peers before
enacting this Wasm. An older host
stays idle on spec 141. The node checks current best-chain state when a worker signs or
hands a bridge transaction to its pool. An already accepted transaction can still
propagate after the halt. A peer advances its Ethereum scan cursor only once it has a
durable retry record or observes the import on chain. After an acknowledgement,
Prometheus reads the halt gauge from the current best block and the peer’s acknowledgement.

A replacement peer needs its bridge key, local RPC settings, and the first unscanned
Ethereum height for each configured network. Restore a digest-pinned cursor export
from a reconciled pre-outage snapshot, or select `"recovery_scan_start": "genesis"`
in `bridge/eth.json` for historical replay. The replacement verifies the source block
against its finalized chain and preserves existing cursors. See the
[operator procedure](../misc/bridge-halt-web/README.md#restarting-the-workers-after-a-halt)
for export, replay, and acknowledgement commands.

## Governance admission

Ordered council and technical committee batches can close multiple motions after
deciding votes. Admission checks a stored proposal’s length before decoding its call.
Validation work for an Operational wrapper has its own transaction-extension weight,
which remains charged if the wrapper stops early and refunds unexecuted dispatches.

## Build and verify

The source commit is `3fd018e0af111106a842566f5cb98ac9d0f6c65e`. Use the pinned Rust
toolchain and a dedicated source checkout. The verified mainnet `generated.rs` used
for spec 138 has SHA-256
`b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85`. A build with
the repository's empty policy cannot author or import the first mainnet block.

```sh
git checkout 3fd018e0af111106a842566f5cb98ac9d0f6c65e
# Set SORA_VERIFIED_RECOVERY_POLICY to the verified generated.rs file.
test "$(shasum -a 256 "${SORA_VERIFIED_RECOVERY_POLICY:?}" | cut -d " " -f 1)" = \
  b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85 || exit 1
cp "$SORA_VERIFIED_RECOVERY_POLICY" runtime/src/incident_recovery/generated.rs
WASM_BUILD_TYPE=release scripts/with_llvm_env.sh \
  cargo build --locked -p framenode-runtime --features build-wasm-binary
```

The output is `target/debug/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm`.
This build imports the same 55 host functions and memory entries as spec 140. Source
paths can affect the Wasm bytes. A rebuilt candidate needs its own fork rehearsal
before governance calls can be prepared for it.

## Repeat the rehearsals and generate unsigned calls

Run from the repository root. The default fork uses finalized mainnet block 28,296,247
(`0x21ca4145bf9ee732a8508f0a99f8c877e5111b130401a2aa5f2d81600b6a1c3d`).
Use `--block <hash>` for another canonical finalized block or `--latest-finalized` for
a fresh preflight. Fresh upgrades from specs 139 and 140 verify the existing guardian
state. Rehearsals use local mock signatures and read-only public RPC.

```sh
(cd runtime-upgrade-4.8.12/validation && npm ci)
node --test runtime-upgrade-4.8.15/validation/release-validation.test.cjs
candidate=runtime-upgrade-4.8.15/framenode-runtime-4.8.15.compact.compressed.wasm
node runtime-upgrade-4.8.15/validation/live-upgrade.cjs "$candidate" rehearsal.json
node runtime-upgrade-4.8.15/validation/live-upgrade.cjs --via-140 "$candidate" via-140.json
node runtime-upgrade-4.8.15/validation/prepare-governance.cjs \
  --rehearsal rehearsal.json "$candidate" unsigned-calls
python3 misc/runtime_upgrade/tests/test_runtime_141_package.py
```

Give each generation a new or empty output directory. The encoder verifies the direct
rehearsal’s Wasm digest, genesis, finalized base height and hash, baseline code and
policy commitment against mainnet. It encodes with finalized metadata and publishes a
complete set of calls through a directory rename. A failed or concurrent generation
cannot mix calls from different candidates in the same directory.

The guarded council call was prepared for 4 of 8 members, and the technical committee
fast-track for 3 of 4. Verify current membership, proposal queue and preimage
status before submission. The package records those values in `governance-calls.json`.
No governance transaction was signed or submitted.
