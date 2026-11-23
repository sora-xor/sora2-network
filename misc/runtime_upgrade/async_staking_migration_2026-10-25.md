# SORA Nexus asynchronous staking migration — 2026-10-25

Implemented and reverified after the bridge readiness fixes. Runtime version **137**, transaction version **133**. No production transactions were submitted and no deployment was performed.

The later [build-verification fix](build_verification_fix_2026-10-25.md) regenerated this identical runtime with controlled build settings and proved the Wasm exports were recreated. Full Clippy and all 24 Wasm stages passed again. The 943 native Rust test executions below retain their original dates and unchanged source bindings.

The runtime uses the actual `pallet-staking-async` 0.16.0 from Polkadot SDK `polkadot-stable2609`, commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`. The Rust dependency alias remains `pallet_staking`. The synchronous pallet remains only in upstream development/benchmark dependency graphs and retained historical source.

## Preserved custom behavior

The [local session bridge](/Users/takemiyamakoto/dev/sora2-network/runtime/src/staking_async.rs) connects the async election and session-report interfaces to the existing BABE/Session chain. It retains the six-session era schedule, one-session authority queue, and existing multi-phase election submission machinery. Tests cover actual session rotation, planned-era activation, forced/fallback election paths, and the first genesis timestamp.

VAL replacement rewards retain existing budgets and reward-point attribution, transactional settlement, replay protection, and fee handling. They do not mint or compound native XOR staking rewards. Historical claims and existing controller relationships remain intact.

Existing **Staking** call, event, error, and storage encodings are preserved. Three bounded-container metadata wrappers have narrowly checked equivalence; all 4,733 pinned values across the affected query prefixes preserve client JSON and full SCALE bytes. The custom exposure-page wrapper exposes the original public metadata fields. The broader SDK upgrade intentionally changes `System.CodeUpdated` and requires clients to refresh metadata.

[Activation](/Users/takemiyamakoto/dev/sora2-network/runtime/src/staking_async_migration.rs) installs storage version 18, activation markers, validator recency, and bridge state. It validates historical state before writing. Historical offences through the already-planned cutover era retain the original span/max calculation, all reporters, reward-halving formula, and frozen-vector cancellation indices. New historical reports use individually keyed records. Bounded processing carries overdue queues across era boundaries and blocks withdrawals from bypassing due slashes. The custom nonzero defer delay retains its additional era.

Denomination now scales paged exposures, recomputes rounded era totals, and covers frozen, historical, and native async slash queues. It preserves queue identities, claims, reporters, and VAL reward budgets.

## Intentional upstream economic changes

Offences after the cutover era use upstream async slashing. Increments are calculated per validator and offence era; nominator liabilities are no longer pooled through the old cross-validator span maximum. New async reports retain the first reporter and reward a fraction of the incremental slash, replacing the historical span-halving formula. Historical commitments keep their original calculation. These differences belong in governance release notes.

## Final verification

All results are bound to the same final source and candidate. The [evidence summary](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/rehearsal-summary.json) independently checks the underlying logs, source hashes, candidate identity, and recorded state witnesses.

| Gate | Result |
| --- | --- |
| Clippy | Production, try-runtime, and extended workspace/all-target modes passed with `-D warnings` |
| Formatting | `cargo fmt --all --check` passed |
| Rust tests | 943 passed; 3 pre-existing ignored; 0 failed |
| Runtime / pallet suites | 298 runtime + 2 local remote-mode-name + 300 async staking + 293 EthBridge + 45 custom multisig tests passed |
| Native production replay | 4 mode checks + fresh pinned-state migration pre/post rehearsal passed |
| Production Wasm rehearsals | All 24 stages passed, including signing, equivocation, fee policy, payout, complete bridge payload recovery, and fresh bridge readiness |
| Staking state witness | All 21,509 captured keys inspected at initialization and finalization; 21,507 existing keys byte-identical |
| Client queries | 4,733 values preserve JSON and SCALE roundtrip; 0 unreviewed metadata changes |
| Rejected transactions | 28 rejected transactions / 69 raw diff entries inspected; no committed protected-state changes; 11 negative/rollback tests passed |
| Dependency audits | 11 maintained Cargo locks + 5 harness locks + 8 npm graphs clean; 10 installed JavaScript checks passed |
| Source provenance | Full async Rust source reconstructed from the exact upstream Git objects plus the recorded patch |

The two expected existing-key changes are the storage version and normal block-authorship reward points (+20). All 19 migration writes and the bridge seed are independently checked. The native upgrade reports execution weight `163589442000` and proof size `12620392`.

Candidate: `535b6499c78769a62701520d0bdf17ef65f07319450ca69a5264e99410e1910f`, 1,959,169 bytes. Source inventory: `dbcaa1324c3e6ec4b2114da9148d24398f00db115037e3e4378fe9daa056a94d`, 4,806 files. RustSec database: `21d909e4c8bd4e14c7aea58bf980c881a0053ab1`; audits report zero vulnerabilities and advisory warnings.

The approved read-only source is `wss://mof2.sora.org`, pinned to block 28,179,122, hash `0x32fd0d8dd97744f86dcf9d964d7fcd25fea43ab5e6d9d7054db9b7b75eba8cd0`. The finalized-runtime guard confirmed that the pinned runtime code remained representative. Earlier SDK evidence stays historical; it is not substituted for the async candidate's tests.

## Limits before activation

These are local rehearsals against pinned public state, not validator deployment or full consensus execution. The native snapshot covers selected pallets and keys; it does not establish global `try_state` invariants over omitted accounts. Public RPC finality/state assertions are trusted rather than independently proven by consensus.

The corrected complete bridge inventory contains **12 stored pending calls and 2,049 hash-only operations**. All 2,061 exact preimages are now recovered and at most 278 bytes. The entire 1,357-entry global stored-call map is within the configured bound. The earlier scanner incorrectly treated a numeric RPC result as optional; that bug is fixed. Both deployed and candidate runtimes reject the same 2,049 obsolete encodings, while 12 decode identically. Unsigned cancellation calls are prepared only for the obsolete entries. Operator readiness and actual cleanup remain unconfirmed; no activation or cleanup was performed. See the [bridge readiness follow-up](/Users/takemiyamakoto/dev/sora2-network/misc/runtime_upgrade/bridge_activation_readiness_2026-10-25.md) for the fixes, current finalized scan, recovery evidence, and remaining operator facts.

On-chain admission requires historical exposures of at most 256 nominators and at most one page per validator when paged, at most 1,200 validator records per era, and frozen slash values no larger than 512 KiB per era. The pinned state has no frozen unapplied slash vectors and at most 42 nominators per exposure page. State outside admission bounds requires a staged migration. Added compatibility weights include conservative database, proof-byte, and decoding charges; they are not newly generated benchmark measurements.

The [fork policy and provenance](/Users/takemiyamakoto/dev/sora2-network/vendor/polkadot-sdk/frame/staking-async/SORA-PATCH.md), [native execution evidence](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/native-remote-execution.json), and [Wasm run manifest](/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation/rehearsal-run.json) contain the detailed contracts and limitations.
