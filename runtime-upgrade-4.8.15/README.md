# SORA Nexus 4.8.15 upgrade candidate

Runtime spec 141, transaction version 134. The corrected candidate succeeds the
published 4.8.14 release (spec 140). The package contains the mainnet Wasm and
unsigned governance calls.

| Artifact | Verified value |
| --- | --- |
| Mainnet Wasm | 2,135,814 bytes, SHA-256 `449d4f231091a7f7230c6e6b61ca0aca797b44ff36424ba5604b96d16481d5e0` |
| `system.setCode` preimage | 2,135,820 bytes, BLAKE2-256 `0xf22f51846c2766204d4f40f83f3c9c0a69149cd6d2d55263e5eb4a5cce760845` |
| Governance metadata | Finalized block 28,299,621, SHA-256 `0d8aaf8c1f6974e2256a0b7bb71c16d1fb502c1d1327e6ca46c44e0f554af3c2` |

All 19 fork checks passed. The fork applied runtime 141 at block 28,296,248,
installed the guardian, preserved native issuance and Root's latch, and accepted a
fee-free guardian halt. The unauthorized and cooldown cases were rejected.
The council threshold was 4 of 8 and the technical committee threshold was
3 of 4. No governance transaction was signed or submitted.

## Bridge peers

Upgrade HASHI peer nodes to the 4.8.15 binary before enacting this Wasm. The node
checks its current best chain under the block import lock when an offchain worker uses
a bridge signing key or hands a bridge transaction to the local pool. A transaction
accepted before a halt may still propagate or be included afterward. On spec 141,
workers on hosts without the guard stay idle.

A guardian halt records its number in the worker gauge even when Root's recovery latch
also stops the peer. After the latest halt finalizes and operations can resume, start
each peer with its current HASHI key, recovery chain specification, local HTTP RPC,
and chain-specific `bridge/eth.json`. A replacement peer prepares its offchain database
when explicitly started with `--offchain-worker always`. The following command acknowledges halt 3.

```sh
framenode <usual recovery flags> --offchain-worker always --bridge-resume-after-halt 3
```

Each eligible signing key starts its own 120-second outgoing approval delay. Pending
requests observed under an older peer's timer start a fresh delay for the new key.
Council and technical committee members can batch a deciding vote and its motion close.
The runtime applies the motion's index, length and weight bounds to the projected tally.

## Build and verify

The source commit is `cf00668b5fb4c1185faadb287b3f6f8b76e39bf2`. Use the pinned Rust
toolchain and a dedicated source checkout. The verified mainnet `generated.rs` used
for spec 138 has SHA-256
`b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85`. A build with
the repository's empty policy cannot author or import the first mainnet block.

```sh
git checkout cf00668b5fb4c1185faadb287b3f6f8b76e39bf2
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

## Repeat the rehearsal

Run from the repository root with the pinned validation dependencies. The replay
defaults to finalized mainnet block 28,296,247
(`0x21ca4145bf9ee732a8508f0a99f8c877e5111b130401a2aa5f2d81600b6a1c3d`). An archive
endpoint must retain that block. The script also accepts an explicit `--block <hash>`
or `--latest-finalized` for a new preflight. Public RPC is read-only. The fork builds
blocks and exercises the guardian halt with local mock signatures.

```sh
(cd runtime-upgrade-4.8.12/validation && npm ci)
node --test runtime-upgrade-4.8.15/validation/release-validation.test.cjs
node runtime-upgrade-4.8.15/validation/live-upgrade.cjs \
  runtime-upgrade-4.8.15/framenode-runtime-4.8.15.compact.compressed.wasm rehearsal.json
node runtime-upgrade-4.8.15/validation/prepare-governance.cjs \
  --rehearsal rehearsal.json \
  runtime-upgrade-4.8.15/framenode-runtime-4.8.15.compact.compressed.wasm unsigned-calls
```

The encoder requires a successful rehearsal for the exact candidate and verifies that
the chosen finalized block still runs the rehearsed baseline Wasm. It encodes with the
metadata from that block. The council call proposes a guarded external-majority motion. The technical committee
call proposes fast-tracking it. Verify the current membership,
queue and preimage status before use. The hashes, member thresholds, block and call
files are recorded in `governance-calls.json`.
