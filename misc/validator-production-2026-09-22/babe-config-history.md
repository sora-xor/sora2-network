# Historical BABE configuration discrepancy

The public chain demonstrates a longstanding discrepancy between the BABE configuration returned by the runtime and the secondary-slot type used by canonical blocks. This is a concrete consensus configuration issue. It does not yet establish which configuration any affected validator's local BABE epoch cache holds.

## Recorded boundaries

Read-only requests used `https://mof2.sora.org`. The endpoint's genesis hash was verified as `0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5`. Full raw probes, hashes, API versions, headers, and chain timestamps are in `babe-config-history.json`.

| Observation | Before | After |
| --- | --- | --- |
| Runtime `BabeApi_configuration` changes from Plain to VRF | Block 5,377,114, spec 33, 2022-05-18 12:20:36 UTC | Block 5,377,115, spec 35, 2022-05-18 12:20:48 UTC |
| `Babe.EpochConfig` changes from absent to VRF | Block 8,035,060, spec 42, 2022-11-21 13:23:06 UTC | Block 8,035,061, spec 42, 2022-11-21 13:24:42 UTC |

Both sides of both boundaries advertise BABE runtime API version 2. The genesis runtime also advertises version 2 and returns `PrimaryAndSecondaryPlainSlots`, with `c=(1,4)`, a 6,000 ms slot duration, and a 600-slot epoch. This discrepancy is not a client incorrectly decoding the older version-1 `secondary_slots` boolean.

The configuration returned at the first boundary ends in SCALE enum byte `02` (VRF), versus `01` (Plain) before it. The later storage value is `0x0100000000000000040000000000000002`, representing `c=(1,4)` and VRF secondary slots.

The API-change block 5,377,115 itself carries a `SecondaryPlain` pre-digest. The storage-migration block 8,035,061 carries a primary pre-digest, and the immediately following canonical block 8,035,062 carries `SecondaryPlain`. Neither transition block's header contains a BABE `NextConfigData` consensus digest. These observations show that the runtime/API changes did not constitute an immediate switch of the canonical consensus slot type. They do not establish that no configuration announcement has ever appeared anywhere in chain history.

The boundary searches assume monotonic absent/present and Plain/VRF transitions in the searched interval. The neighboring boundary blocks were then queried directly; the raw search probes are retained. Current-chain evidence collected elsewhere in this investigation independently shows that canonical Plain blocks and runtime VRF configuration still coexist in September 2026.

## Source explanation

- Commit `1ee521ed243bde79b546a4266beefbcc3726860f` (16 May 2022, “New staging (#660)”) changes `runtime/src/lib.rs`'s `BabeApi::configuration()` from `PrimaryAndSecondaryPlainSlots` to `PrimaryAndSecondaryVRFSlots`.
- Historical `BabeConfigMigration` calls `pallet_babe::migrations::add_epoch_configuration(BABE_GENESIS_EPOCH_CONFIG)`. It appears in commit `680c0e146`, was merged in `38acb6fc2`, and remains in the spec-42 source at `6b43803d7`. That constant specifies VRF. The SDK migration writes `EpochConfig` and `NextEpochConfig` directly; it does not emit `NextConfigData` for this supplied configuration.
- The current `main_net()` loader reads the embedded raw `node/chain_spec/src/bytes/chain_spec_main.json`. That historical genesis has no `EpochConfig` or `NextEpochConfig` entry. New coded-chain genesis constants do not replace this raw mainnet genesis.
- In pinned SDK `e373717`, `substrate/client/consensus/babe/src/lib.rs:1592` computes each next epoch's config from the header's `NextConfigData`, falling back to the previous locally cached epoch's config. Normal epoch import does not refresh the config from runtime storage.
- The persistent local epoch tree is loaded from the node's auxiliary database. Restarting a node with a current-format, intact epoch tree preserves its cached config. Legacy epoch-tree version-0/1 migration instead supplies config from the runtime API (`babe/src/migration.rs:66` and `aux_schema.rs:64`). This differs from a simple restart of a current database.
- Warp state import explicitly resets the local epoch tree from runtime `current_epoch()` and `next_epoch()` (`babe/src/lib.rs:1216`), which read the stored VRF config. Fresh full sync starts from genesis configuration, which is Plain. These are concrete paths by which differently initialized nodes can disagree.
- The client verifier permits `SecondaryPlain` only when the local epoch config allows Plain, and `SecondaryVRF` only when it allows VRF (`babe/src/verification.rs:104`). The runtime block execution itself is not the equivalent of this client consensus check.

The node audit found no active custom networking patch: `Cargo.lock` resolves `sc-network` 0.56.1 and `sc-network-sync` 0.55.1 to SDK `e373717`; the legacy `vendor/sc-network` directory is not selected by the current Cargo patch table. Ordinary finality backoff remains another node-local authorship gate, but it does not explain away the independently observed configuration discrepancy.

## Repair mechanics and limits

The existing root-only `Babe.plan_config_change(V1 { c: (1,4), allowed_slots: PrimaryAndSecondaryPlainSlots })` path provides an announced transition. It writes `PendingEpochConfigChange`. At the next epoch boundary, the runtime first copies the old `NextEpochConfig` into current, then places Plain in next and emits `NextConfigData(Plain)`. At the following boundary, Plain becomes current. Thus current runtime storage takes two subsequent epoch transitions to converge.

The current runtime's `BabeApi::configuration()` also hardcodes VRF independently of storage. A governance config call alone does not repair that API. A code change should make its returned `c` and `allowed_slots` consistent with the intended configuration and preserve correct historical-genesis behavior.

Nodes already holding incompatible local epoch state may reject a canonical Plain block before reaching an announcement. The announced transition must not be represented as guaranteed immediate recovery for every affected node. The local epoch state, sync initialization method, and verification errors still need confirmation on an affected validator. No governance call, node restart, database modification, key access, or production change was performed for this historical audit.
