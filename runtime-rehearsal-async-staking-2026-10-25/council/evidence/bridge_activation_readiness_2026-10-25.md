# SORA Nexus bridge activation readiness — 2026-10-25

Technical verification passed on the final runtime candidate. No production transactions, cleanup votes, or deployment have been submitted. The user will coordinate operator upgrades after the final Docker image is published. Operator deployment confirmation is a subsequent rollout check, not a prerequisite for preparing the image.

## Resolved technical gaps

The readiness scanner now reads the raw nullable JSON-RPC storage-size result. The previous scanner incorrectly treated PolkadotJS's numeric `u64` result as an `Option`, causing stored payloads to be classified as absent. At pinned production block **28,179,122**, the corrected complete scan finds **12 stored pending payloads and 2,049 hash-only pending operations**. The entire global `BridgeMultisig.Calls` map contains **1,357 entries**, including orphaned entries; all payloads are at most **278 bytes**, below the **16,384-byte** bound.

All **2,061** pending payload preimages are recovered and checked against their BLAKE2-256 hashes. The retained-request scan covered 230,144 requests; archived extrinsics supplied the remaining 13 preimages. Original block header hashes, ordered extrinsics roots, exact historical extrinsic encoding, and pending timepoints are checked independently. Public storage and metadata responses are trusted; no independent consensus or storage-trie proof is claimed.

**2,049 payloads already fail decoding under the deployed runtime and fail identically under the candidate.** Most use the historical EthBridge pallet index 30 instead of 31; index 30's current DEXAPI call enum has no matching import call 11. The other **12** payloads decode identically under both runtimes. There are no newly rejected or newly enabled pending payloads from this upgrade.

## Code fixes

Bridge offchain submissions and retries now select a current peer for the correct network and, for multisig/cancellation calls, a current signatory. Outgoing retries retain the exact public key embedded in their approval payload. A removed or wrong-network key is rejected without being substituted for that embedded key. Pending transaction encoding and nonce tracking are unchanged.

Legacy stored-call loading now checks the encoded length before allocating the value in Wasm, limits the full tuple allocation and decoder, and rejects malformed or trailing bytes. The runtime's nested-call filter uses the same bounded reader. The historical storage ABI is unchanged. The extra valid-path database read is accounted for conservatively. The host still reads the backend value and its trie proof; the complete global inventory establishes the observed state-specific size bound, and new submissions enforce the configured cap.

## Recovery and operator evidence

The [unsigned cancellation review](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/bridge-cancellation-review.json) is limited to pre-existing incompatible entries. The 12 compatible entries are retained without cancellation calls. Each selected operation requires its current eligible peer quorum and exact timepoint. Cancellation removes pending capacity; it does not execute historical payloads, retry transfers, reset incoming status, or erase replay protection. This file is preparation for review, not authorization or evidence of completed cleanup.

The currently configured network has three multisig members and a quorum of three. Public chain state establishes membership and payload facts, but cannot establish each operator's local peer key availability, offchain-worker/network configuration, external client metadata handling, or recovery readiness. As confirmed by the user on 2026-10-25, operators will upgrade after final image publication; no operator host access is needed to prepare that release. `operatorReadinessConfirmed` remains false until rollout evidence exists; local verification does not authorize activation.

## Docker release and compatibility sequence

The release order is: verify and publish the final immutable image, have operators upgrade and confirm readiness, then activate the runtime through governance. The unsigned obsolete-entry cleanup remains a separate quorum action.

Source review found unchanged local CLI definitions, data paths, bridge keystore identifiers, offchain-worker storage keys, and chain protocol identity. Both the prior SDK lineage and stable2609 use client database schema version 4; no schema-version bump was found. Runtime verification confirms unchanged host import signatures and runtime API versions, preserved staking queries, and no newly incompatible pending bridge payloads. These checks do not establish final Linux binary, database-backend, or mixed-version network compatibility.

`bridge-docker/Dockerfile.sora-release` packages existing amd64 and arm64 binaries from fixed build directories. Those paths alone do not prove that either binary contains the verified source and runtime. Before publication, bind both Linux binaries and their embedded runtime to the verified candidate, check each architecture's final image loader and existing launch arguments, and exercise copied-database reopen/restart, synthetic keystore and offchain-worker configuration/cache persistence, network handshake/synchronization, and operation across the runtime upgrade. Scan the final images as well: the completed RustSec and npm audits do not cover Debian base-image packages. Preserve each operator's configured database backend, base path, keystore, and configuration during rollout. No final image has been verified or published by this follow-up.

Compatibility includes reviewed SDK changes: runtime spec 137 uses transaction version 133, and `System.CodeUpdated` requires clients to refresh metadata. Clients must use the active runtime's signing payload; previously signed transactions are not guaranteed to remain valid across activation. The previously documented intentional new-era slashing changes also remain in scope.

## Final verification

- All three full Clippy configurations passed with `-D warnings`; formatting passed.
- **943 Rust test executions passed**, with 3 pre-existing ignored tests and no failures. This includes all 293 EthBridge and 45 custom multisig tests, the runtime/async staking suites, and a fresh native migration replay with pre/post checks.
- All **24 production Wasm rehearsal stages passed**. All 108 bridge evidence tests passed: 62 readiness-policy cases, 13 raw RPC size cases, 16 payload recovery cases, and 17 capture-lineage cases.
- The fresh finalized bridge scan at block **28,187,530** (`0xb6a21a7b5773e761c8ec1e06c9e55eea3a8d89ea9ef0c7b8fc2b1bdd5667a663`) confirmed the same 2,061 pending operations and 1,357 global stored calls, with no oversized payloads, no newly rejected payloads, and no newly enabled legacy payloads.
- All 11 maintained Cargo locks, 5 retained harness locks, and 8 npm dependency graphs audited clean; 10 installed JavaScript checks passed.
- Independent aggregation rechecked source/candidate hashes, raw state witnesses, 28 rejected transactions, all 2,061 preimages, and every one of the 4,102 extracted request records against the complete 450-page raw request capture.

Runtime spec **137**, transaction version **133**. Candidate SHA-256: `535b6499c78769a62701520d0bdf17ef65f07319450ca69a5264e99410e1910f` (1,959,169 bytes). Source SHA-256: `dbcaa1324c3e6ec4b2114da9148d24398f00db115037e3e4378fe9daa056a94d`.

The [final verification summary](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/rehearsal-summary.json), [current bridge scan](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/bridge-activation-current.json), and [payload recovery proof](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/bridge-payload-recovery-verification.json) contain the recorded evidence. After the [build-verification fix](build_verification_fix_2026-10-25.md), full Clippy, the production build, and all 24 Wasm stages passed again; the current bridge scan advanced to block **28,191,008** with the same readiness conclusions. Native test and audit executions retain their original dates and bindings. The candidate is unchanged. Refreshed verification summary SHA-256: `6e34a7276e1ca733d9069bfacc1edeb1b42ae0bcbe320f4fdb348b05238dd379`.

The migration retains its previously documented intentional new-era slashing changes and scoped-replay limitations. A passing local verification result does not certify operator configuration or execute the unsigned cleanup plan.
