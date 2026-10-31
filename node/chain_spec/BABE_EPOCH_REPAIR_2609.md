# SORA Nexus BABE cache preparation on stable2609

Local source comparison on 2026-10-26 revalidated the cache reader/writer against
`polkadot-stable2609`, commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`.
The retained runtime source uses stable2603-3 (`e373717`). The following SDK files
are byte-for-byte identical between those revisions:

| Source | SHA-256 in both revisions |
|---|---|
| `substrate/client/consensus/babe/src/aux_schema.rs` | `92689c586a51c8206e4d7ecf2980cd3801bbaa8d153c44271b24678dc0505c65` |
| `substrate/client/consensus/epochs/src/lib.rs` | `d03ab089a407d2d5b914d35cec14c1d2fb9ff55fdf8d3547dc0c9928f191cfdc` |
| `substrate/primitives/consensus/babe/src/lib.rs` | `d9bd66cd4ad5efe8dbd7eb13274a5f88b98d5da0026ec5572716e90179295cec` |
| `substrate/client/db/src/upgrade.rs` | `508733d5b2280213fc43a928a4d75bb3cf3520604616a5c45275dfab0bcdf2e4` |

The stable2609 backend still has 13 columns, AUX column 8, RocksDB schema 4 and
BABE auxiliary epoch schema 3. Its ParityDB full-backend column options still use
LZ4 for columns 1/4/5/12/11/6 and ref-counted/preimage/uniform options for 1/11,
matching `GuardedDatabase`. The guarded command refuses other formats, directory
migration, genesis creation and unrelated logical writes. Only the exact cache
replacement authorized by its durable backup may be committed.

Recovery preparation now additionally refuses a database whose best or finalized
head reaches 28,191,817. Run it against the separate restored database ending at
28,191,816 before checkpoint import. This tool does not reinterpret abandoned or
recovery epochs; the node's immutable substitute and forced GRANDPA boundary
handle the checkpoint and its descendants.

The complete node test suite subsequently passed on stable2609 (39 tests on
2026-10-27), including the real ParityDB and RocksDB fixtures. They verify exact
single-key cache authorization, backup-before-write, retained unrelated data,
reopening, incompatible-layout rejection and preservation of existing missing-body
gaps. This qualifies the implementation against both SDK database engines; the
exact operator-restored database and final release binary still require their
own rehearsal.
