# SORA Nexus 4.8.14 runtime upgrade

This directory records the published 4.8.14 release. For the corrected runtime and
bridge node, use [4.8.15](../runtime-upgrade-4.8.15/README.md). The validation entry
points here use the current rehearsal and digest checks for this historical candidate.
The published 4.8.14 ZIP and its checksum identify the original release at its signed tag.

Runtime spec **140**, transaction version **134**. It upgrades the live runtime 138.

| Item | Value |
| --- | --- |
| Wasm | `framenode-runtime-4.8.14.compact.compressed.wasm`, 2,143,899 bytes |
| SHA-256 | `0d9c61bc3142318f595404cc7809128a11beaf80369660688d9b740d08517b7e` |
| BLAKE2-256 | `0x4c6b2f37c6d98e4ee256e9c53dc85894033d2c2bbce83080306d705f83601852` |
| `system.setCode` proposal | `0x43db4b428b9b3e9295e35ee9dd2a8eb494bfd84e694772e581b6777cfec4c4c0`, 2,143,905 bytes (`set-code-call.scale`) |
| Source | branch `4.8.14`, commit `34663ad22e7085a8c0daba6c8071a45e13ed95f2` |
| Recovery policy | commitment `0x1f4e11427a9c0a61a4868af40c9e0bdf263bb44a9ee59886a537c52164f3cdbb`, the compiled policy of runtime 138 |

## What changes since runtime 138

- **Guardian bridge halt.** The new BridgeHalt pallet (index 122) lets governance-appointed
  guardians halt the bridge workers with one fee-free transaction. Each halt is numbered.
  The HASHI offchain worker on every bridge node stops when it sees a halt and stays
  stopped until devops restart that node with `--bridge-resume-after-halt <number>`; no
  Root call is needed to bring the bridge back. Halts are at least 10 blocks apart, and a
  guardian's halt ranks above every other transaction in the pool. The upgrade installs
  the first guardian, `cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY`. The halt page
  is at https://emergency.sora.org.
- Operational-class calls that their signer can never make are rejected before inclusion.
- Each HASHI peer waits 120 seconds after an outgoing request is final before signing it.
- A batch of validator payouts pays the direct payout fee for each claim.

Root's recovery latch, which keeps the bridges halted since the October 2026 recovery,
is not changed by this upgrade.

## Build

The repository keeps `runtime/src/incident_recovery/generated.rs` empty on purpose. A
mainnet build must supply the compiled recovery policy that runtime 138 was built with
(that `generated.rs` has SHA-256
`b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85`). Without it the
runtime stops in its first block on mainnet with `MissingCompiledPolicy`. Never commit
the policy file.

```sh
git checkout 34663ad22e7085a8c0daba6c8071a45e13ed95f2
cp <runtime 138 policy>/generated.rs runtime/src/incident_recovery/generated.rs
WASM_BUILD_TYPE=release scripts/with_llvm_env.sh \
  cargo build --locked -p framenode-runtime --features build-wasm-binary
# target/debug/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm
git checkout -- runtime/src/incident_recovery/generated.rs
```

`WASM_BUILD_TYPE=release` makes the debug build write the compact, compressed blob. The
Wasm embeds source paths, so a byte-identical rebuild needs the same toolchain and
checkout path.

## Tested against live mainnet

`validation/live-upgrade.cjs` forked finalized block 28,296,247
(`0x21ca4145bf9ee732a8508f0a99f8c877e5111b130401a2aa5f2d81600b6a1c3d`) from
`wss://mof2.sora.org` with the vendored Chopsticks, installed this exact Wasm and built
blocks on top. Public RPC was read-only and nothing was submitted to the network. All 19
checks passed; `live-fork-rehearsal.json` records them.

- The upgrade block applied runtime 140 with no failed inherent. The guardian migration
  installed exactly `cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY`, and every other
  migration found its work already done. The block used 128.5 ms of mandatory weight
  out of 2 s.
- Native XOR issuance and Root's recovery latch were unchanged, and the next block built
  normally on runtime 140.
- A halt from the real guardian account (mocked signature, funded on the fork) was
  included as halt #1 with a zero fee. The bridge workers' signal read `(1, 28296250)`,
  and `bridgeHalt.lastHalt` recorded the number, guardian and block.
- A halt from another account was rejected as `BadSigner`, and a second halt inside the
  cooldown as `Stale`, both before inclusion.

The Wasm imports the same 55 host functions as runtime 138, so nodes that have not
upgraded keep running. Native tests on the source commit passed: eth-bridge 448,
bridge-halt 11, common 169, runtime 525 (6 ignored, as before) and node 64.

To repeat the fork check:

```sh
(cd runtime-upgrade-4.8.12/validation && npm ci)
node runtime-upgrade-4.8.14/validation/live-upgrade.cjs \
  runtime-upgrade-4.8.14/framenode-runtime-4.8.14.compact.compressed.wasm
```

## Governance

The calls below are unsigned. They were encoded from finalized block 28,296,249
(`0xfc9a5681a90a0a70f25c218a755b144ed16dccde5f967639d4d4f82ef7110d7c`), when the council
had 8 members, the technical committee 4, and no external proposal was queued. Nothing
has been signed or submitted. `governance-calls.json` lists every call with its length
and hash.

1. Note the preimage with `preimage-note-call.hex` (`preimage.notePreimage` of the
   `system.setCode` call, 2,143,911 bytes).
2. Council motion, threshold 4 of 8: `council-propose-guarded-external-majority-threshold-4-call.hex`.
   It proposes `utility.batchAll([democracy.externalPropose, democracy.externalProposeMajority])`
   for the proposal hash above. The batch fails, changing nothing, if another external
   proposal is already queued.
3. Technical committee motion, threshold 3 of 4: `technical-committee-fast-track-threshold-3-call.hex`,
   which is `democracy.fastTrack(proposal, 1800, 0)`.
4. The referendum runs 1,800 blocks (about 3 hours) and enacts right after it passes.

If council membership, the queue or the live runtime changes, regenerate the calls with
`node runtime-upgrade-4.8.14/validation/prepare-governance.cjs <wasm> <output dir>`.

## Operators

Bridge (HASHI peer) nodes need the 4.8.14 `framenode` build to restart their workers
after a halt with `--bridge-resume-after-halt <number>`. On an older build, a halt still
stops the workers, but they stay stopped until the node is upgraded. Validators need no
node change for this runtime upgrade.

`SHA256SUMS` covers every file in this folder.
