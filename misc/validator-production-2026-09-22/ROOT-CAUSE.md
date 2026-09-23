# Confirmed BABE epoch configuration divergence

The chain advertises **PrimaryAndSecondaryVRFSlots** in runtime state and all three BABE configuration APIs, but its finalized consensus chain uses **PrimaryAndSecondaryPlainSlots**. This is a confirmed protocol configuration defect. An actual finalized header fails the pinned SDK verifier under the runtime-advertised epoch and passes under the same epoch with only the secondary slot mode corrected to plain.

This explains a concrete way a state-synced validator can stop following or producing on the canonical chain. It does not establish which of the four affected validators underwent that path: their local epoch caches, sync histories, and logs have not been inspected.

## Reproduction with a real finalized header

[`canonical-header.json`](canonical-header.json) records block **27,735,177**, hash `0x10fdab9d0ed95dc57ddb86163aa88f678a2e7d391743ce91806e6990d97f1391`, in epoch **47,369**. Its BABE pre-digest is `0x02050000007f64c81100000000`: variant **2** (`SecondaryPlain`), authority index **5**, slot **298345599**. The original seal is retained.

At the exact same block, [`babe-config-evidence.json`](babe-config-evidence.json) records:

- `Babe.EpochConfig` and `Babe.NextEpochConfig`: raw SCALE `0x0100000000000000040000000000000002`. These are two little-endian u64 values `(1,4)` and `AllowedSlots` enum index **2**, meaning **PrimaryAndSecondaryVRFSlots**.
- `BabeApi_configuration`, `BabeApi_current_epoch`, and `BabeApi_next_epoch`: the same VRF slot mode. No planned change is pending.
- Twenty recent finalized headers: **13 SecondaryPlain**, **7 Primary**, **0 SecondaryVRF**.

The diagnostic imports `verification.rs` and `authorship.rs` unchanged from pinned SDK commit `e3737178ec726cffe506c907263aaaa417893fd0`; it uses existing compiled release libraries. No cryptography is mocked. The positive control checks the real header hash, deterministic secondary authority assignment, and sr25519 seal. Results are saved in [`canonical-verifier.json`](canonical-verifier.json) and [`canonical-verifier.log`](canonical-verifier.log):

| Input epoch mode | Actual SDK result |
| --- | --- |
| Captured runtime VRF mode | `SecondarySlotAssignmentsDisabled` |
| Same epoch, only mode changed to Plain | `Checked` |

Run locally with `python3 misc/validator-production-2026-09-22/run-canonical-verifier.py`. Adding `--expect-runtime-acceptance` asserts the desired invariant that canonical headers verify using the advertised epoch; this deliberately exits **101** on the captured faulty state. The script requires the documented local warm target and SDK checkout, invokes no Cargo build or network, and submits nothing.

The enum encodings and rejection are explicit in the pinned SDK: [PreDigest](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/primitives/consensus/babe/src/digests.rs#L73), [AllowedSlots](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/primitives/consensus/babe/src/lib.rs#L235), and [header verification](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/client/consensus/babe/src/verification.rs#L104).

## Why nodes can disagree

BABE clients normally maintain an auxiliary epoch tree independently of runtime storage. On ordinary restart, the client restores that tree. For each imported epoch announcement, it inherits the previous cached slot mode unless a `NextConfigData` consensus digest explicitly announces a change. A long-lived plain-mode client therefore continues following the plain-mode chain even if runtime storage separately says VRF. See [auxiliary cache loading](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/client/consensus/babe/src/aux_schema.rs#L58) and [configuration inheritance](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/client/consensus/babe/src/lib.rs#L1585).

State import after warp/state sync instead reconstructs the epoch tree from `current_epoch()` and `next_epoch()` in the imported runtime. Those APIs read the incorrect on-chain configuration. Such a node gets VRF mode and rejects the next plain secondary block with the reproduced error. See [state import and epoch reset](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/client/consensus/babe/src/lib.rs#L1193) and [runtime epoch APIs](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/frame/babe/src/lib.rs#L737).

There is another conditional path: migrations of very old BABE auxiliary database versions 0/1 fill the previously absent epoch configuration from `BabeApi_configuration()`. That SORA API currently hardcodes VRF instead of reading `Babe::epoch_config()`. See [old cache migration](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/client/consensus/babe/src/migration.rs#L69). A normal restart of a recent valid plain-mode cache alone does **not** cause this reset.

## Repair design

Preserve the slot mode that already validates the canonical chain. Schedule `Babe.plan_config_change(V1 { c: (1,4), allowed_slots: PrimaryAndSecondaryPlainSlots })` through the existing root-authorized BABE mechanism. At the first normal epoch boundary BABE announces `NextConfigData(Plain)` and stores the next configuration. At the second boundary the active runtime configuration becomes Plain. Existing canonical clients see a valid explicit announcement and retain their existing mode. See [planning semantics](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/frame/babe/src/lib.rs#L454) and [announcement/application](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/substrate/frame/babe/src/lib.rs#L711).

The local runtime repair must also make `BabeApi_configuration()` read the active stored probability and slot mode, with the configured genesis fallback only when storage is absent. That is the pattern used by the pinned [Westend runtime](https://github.com/paritytech/polkadot-sdk/blob/e3737178ec726cffe506c907263aaaa417893fd0/polkadot/runtime/westend/src/lib.rs#L2604), and allows future legitimate planned configuration changes.

A one-time migration can schedule the correction only on the known SORA mainnet genesis, only for the exact observed current+next VRF `(1,4)` state, and only when no other change is pending. Unknown states and existing plans must be preserved. It must not silently overwrite active/next configurations or change validator keys, authorities, randomness, or VRF/signature verification. The repair is effective after the two normal epoch transitions; nodes with an already divergent cache may require operator-controlled state recovery after the corrected state is available. No validator database should be deleted as an automatic action.

## Release compatibility checks

The 4.8.8 artifact SHA-256 is `0ac7b85d845d56d450df99ab7166014897be57a7a1f66437d92e6949b56f49c0`; deployed 4.8.9 is `db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447`. Offline parsing of both decompressed artifacts found identical **55 imports**, including their function signatures, **86 exports**, `runtime_apis`, `target_features`, and `producers` sections. No new host-function requirement or runtime API version explains the issue.

Source comparison `411dcdb70..282f4679b` keeps the same SDK revision and BABE authoring service. Node service changes are lint attributes; the node uses `WasmExecutor`. Node/runtime Cargo feature definitions and release build scripts are unchanged. The runtime specification and transaction versions increase from 130 to 131, while authoring version remains 1. These checks do not identify the binaries running on affected validators.

The runtime upgrade executed at block 27,693,182 on 18 September 2026 at 18:49:30 UTC. Indices 2 and 10 subsequently authored under spec131 on 22 September at 05:08:18 and 08:45:30 UTC respectively; see [`author-stop-trace.json`](author-stop-trace.json). Thus their later stops cannot be explained by a universal inability to execute runtime4.8.9 immediately after activation.
