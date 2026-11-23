# SORA Nexus 4.8.14 runtime upgrade

Runtime spec 141, transaction version 134. The package contains the mainnet Wasm and
unsigned governance calls for the upgrade from the finalized runtime, spec 138.

| Artifact | Verified value |
| --- | --- |
| Mainnet Wasm | `framenode-runtime-4.8.14.compact.compressed.wasm`, 2,151,959 bytes, SHA-256 `125e6582db89251a9b0ebe3f3549548d9addc7f4b83531d17239bc9e99403b9f` |
| `system.setCode` preimage | 2,151,965 bytes, BLAKE2-256 `0xb0946dc57de622727151a3298aeed7b3833c55add1e126d432f8bf373e8f3195` |
| Governance metadata | Finalized block 28,315,517, SHA-256 `0d8aaf8c1f6974e2256a0b7bb71c16d1fb502c1d1327e6ca46c44e0f554af3c2` |
| Source | branch `4.8.14`, commit `5da069985dfa574bc2c1c030853f305fde5191d0` |

An earlier cut of 4.8.14, runtime spec 140, was never enacted and this package replaces
it. Its files are kept in [`runtime-upgrade-4.8.14-spec-140`](../runtime-upgrade-4.8.14-spec-140/README.md)
for the successor rehearsal below. Do not submit anything from that directory.

## What changes since runtime 138

- **Guardian bridge halt.** The new BridgeHalt pallet (index 122) lets guardians halt the
  bridge workers with one fee-free transaction. Each halt is numbered. The HASHI offchain
  worker on every bridge node stops when it sees a halt and stays stopped until devops
  restart that node with `--bridge-resume-after-halt <number>`. Halts are at least 10
  blocks apart, and a guardian's halt ranks above every other transaction in the pool.
  The upgrade installs two guardians, `cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY`
  and `cnU8UgdM4fs9iiCxkRDW8vp2bx4e68LbePkcuQPv1LERKH5oH`. The technical committee (more
  than 2/3) or Root changes the list afterwards. The halt page is at
  https://emergency.sora.org.
- **Operational admission.** Operational-class calls that their signer can never make
  are rejected before inclusion. See [Governance admission](#governance-admission).
- **Signing delay.** Each HASHI peer waits 120 seconds after an outgoing request is
  final before signing it.
- **Payout batches.** A batch of validator payouts pays the direct payout fee for each
  claim.
- **Recorded attacker identities.** Incoming bridge delivery redirects only transfers
  that involve the recorded attacker account, its Ethereum address, or an identity Root
  adds to the attacker registry. A second account that runtime 138 hard-coded by
  mistake is no longer treated that way.

Root's recovery latch, which has kept the bridges halted since the October 2026
recovery, is not changed by this upgrade.

## Rehearsals

All three rehearsals forked finalized mainnet block 28,315,359
(`0x4d9d2c1660361839978c14c7355b589fb52d40f810fe596ebb959141d1c7620d`) with the vendored Chopsticks. Public RPC was read-only, signatures were
mocked, and nothing was submitted to the network.

**Direct upgrade, 21 checks passed** (`live-fork-rehearsal.json`). The script verified
genesis, the canonical finalized block, the deployed spec-138 Wasm and the recovery
policy. The fork installed spec 141 and the migration seeded exactly the two guardians
above; the upgrade block used 128.7 ms of mandatory weight out of 2 s. Native XOR
issuance, the recovery policy and Root's latch were unchanged. The first guardian halted
fee-free as halt 1, a non-guardian was rejected before inclusion, and a second halt
inside the cooldown was refused. After the cooldown the second guardian halted fee-free
as halt 2. Only this rehearsal qualifies governance calls.

**Successor upgrade, 19 checks passed** (`via-140-rehearsal.json`). The fork first installed
the superseded spec-140 Wasm, assigned a replacement guardian and recorded halt 1.
Spec 141 preserved that guardian list, halt record and worker signal byte for byte and
did not seed again. The same guardian then issued halt 2. This shows that the seed
migration leaves an existing guardian list alone.

**Governance on spec 141, 24 checks passed** (`governance-path-rehearsal.json`). With spec 141
installed, the mainnet council (threshold 4 of 8) and technical committee
(threshold 3 of 4) passed a guarded external-majority proposal of
`system.killStorage` for Root's latch and fast-tracked it. The referendum passed and
the scheduler dispatched the Root call, which removed the latch. Calls from non-members
and a close that could not succeed yet were rejected before inclusion; the deciding
council vote and the close went through in one `batchAll`; a runtime-sized preimage
was noted. One local shortcut is recorded in the report: the referendum's end was moved
to the next block instead of building 1,800 blocks.

## Bridge peers

Build the 4.8.14 node from the source commit above and upgrade the HASHI peers before
enacting this Wasm. A node built from the superseded spec-140 cut, or any older node,
stays idle on spec 141. The node checks current best-chain state when a worker signs or
hands a bridge transaction to its pool. An already accepted transaction can still
propagate after the halt. A peer advances its Ethereum scan cursor only once it has a
durable retry record or observes the import on chain. After an acknowledgement,
Prometheus reads the halt gauge from the current best block and the peer's acknowledgement.

A replacement peer needs its bridge key, local RPC settings, and the first unscanned
Ethereum height for each configured network. Restore a digest-pinned cursor export
from a reconciled pre-outage snapshot, or select `"recovery_scan_start": "genesis"`
in `bridge/eth.json` for historical replay. The replacement verifies the source block
against its finalized chain and preserves existing cursors. See the
[operator procedure](../misc/bridge-halt-web/README.md#restarting-the-workers-after-a-halt)
for export, replay, and acknowledgement commands.

## Governance admission

Ordered council and technical committee batches can close multiple motions after
deciding votes. Admission checks a stored proposal's length before decoding its call.
Validation work for an Operational wrapper has its own transaction-extension weight,
which remains charged if the wrapper stops early and refunds unexecuted dispatches.

## Build and verify

Use the pinned Rust toolchain and a dedicated source checkout. The verified mainnet
`generated.rs` used for spec 138 has SHA-256
`b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85`. A build with
the repository's empty policy cannot author or import the first mainnet block. Never
commit the policy file.

```sh
git checkout 5da069985dfa574bc2c1c030853f305fde5191d0
# Set SORA_VERIFIED_RECOVERY_POLICY to the verified generated.rs file.
test "$(shasum -a 256 "${SORA_VERIFIED_RECOVERY_POLICY:?}" | cut -d " " -f 1)" = \
  b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85 || exit 1
cp "$SORA_VERIFIED_RECOVERY_POLICY" runtime/src/incident_recovery/generated.rs
WASM_BUILD_TYPE=release scripts/with_llvm_env.sh \
  cargo build --locked -p framenode-runtime --features build-wasm-binary
git checkout -- runtime/src/incident_recovery/generated.rs
```

The output is `target/debug/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm`.
This build imports the same 55 host functions and memory entries as the live spec-138
runtime. Source paths can affect the Wasm bytes. A rebuilt candidate needs its own fork
rehearsals before governance calls can be prepared for it.

## Repeat the rehearsals and generate unsigned calls

Run from the repository root. Without `--block`, the scripts fork finalized mainnet
block 28,296,247 (`0x21ca4145bf9ee732a8508f0a99f8c877e5111b130401a2aa5f2d81600b6a1c3d`);
pass `--block <hash>` for another canonical finalized block, as this package did, or
`--latest-finalized` for a fresh preflight. Fresh upgrades from specs 139 and 140 verify
the existing guardian state.

```sh
(cd runtime-upgrade-4.8.12/validation && npm ci)
node --test runtime-upgrade-4.8.14/validation/release-validation.test.cjs
candidate=runtime-upgrade-4.8.14/framenode-runtime-4.8.14.compact.compressed.wasm
block=0x4d9d2c1660361839978c14c7355b589fb52d40f810fe596ebb959141d1c7620d
node runtime-upgrade-4.8.14/validation/live-upgrade.cjs --block "$block" "$candidate" rehearsal.json
node runtime-upgrade-4.8.14/validation/live-upgrade.cjs --via-140 --block "$block" "$candidate" via-140.json
node runtime-upgrade-4.8.14/validation/governance-path.cjs --block "$block" "$candidate" governance-path.json
node runtime-upgrade-4.8.14/validation/prepare-governance.cjs \
  --rehearsal rehearsal.json "$candidate" unsigned-calls
python3 misc/runtime_upgrade/tests/test_runtime_141_package.py
```

Give each generation a new or empty output directory. The encoder verifies the direct
rehearsal's Wasm digest, genesis, finalized base height and hash, baseline code and
policy commitment against mainnet. It encodes with finalized metadata and publishes a
complete set of calls through a directory rename. A failed or concurrent generation
cannot mix calls from different candidates in the same directory.

## Governance

The calls are unsigned. `governance-calls.json` lists every call with its length and
hash, and records the membership and queue they were prepared against.

1. Note the preimage with `preimage-note-call.hex` (`preimage.notePreimage` of the
   `system.setCode` call).
2. Council motion, threshold 4 of 8:
   `council-propose-guarded-external-majority-threshold-4-call.hex`. It proposes
   `utility.batchAll([democracy.externalPropose, democracy.externalProposeMajority])`
   for the proposal hash above. The batch fails, changing nothing, if another external
   proposal is already queued.
3. Technical committee motion, threshold 3 of 4:
   `technical-committee-fast-track-threshold-3-call.hex`, which is
   `democracy.fastTrack(proposal, 1,800, 0)`.
4. The referendum runs 1,800 blocks (about 3 hours) and enacts right after it passes.

Verify current membership, the proposal queue and the preimage status before
submission. No governance transaction was signed or submitted.

`SHA256SUMS` covers every file in this folder.
