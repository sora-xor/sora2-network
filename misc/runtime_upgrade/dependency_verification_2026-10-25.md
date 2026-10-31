# SORA Nexus dependency verification — 2026-10-25

Full Clippy and both production-state rehearsal lanes pass on the current dependency graph. The candidate advances deployed spec 134 to **135**, retaining transaction version **131**. No production transactions or deployment were performed.

## Bound candidate and state

- Candidate: `2109096d15dd84e49c7d081ad20ab7b194424cc4e640f804133da0b836c28adf` (1,900,414 bytes), built with production Wasm features only.
- Source: `e7baceebe77829ca94a2a8c79f1a83704b759582570f74836c9d0607321dfb69` across 4,542 files. Complete source checks passed before/after Clippy, build, native tests and fork execution.
- Pinned production block: **28,179,122**, `0x32fd0d8dd97744f86dcf9d964d7fcd25fea43ab5e6d9d7054db9b7b75eba8cd0`, through `wss://mof2.sora.org`. Provider review identified OVH; this is not a validator-hosting assertion.
- Archived 4.8.12 release binaries, signatures and historical evidence were preserved. New evidence lives in a separate rehearsal directory.

## Validation

| Gate | Result |
| --- | --- |
| Full Clippy | `mainnet`, `try-runtime`, `extended`; all targets, `-D warnings`; zero compiler errors or warnings |
| Formatting and source identity | Passed; production feature guard excludes private/stage/WIP/benchmark/try-runtime Wasm |
| Native Rust regressions | **508 passed**, 3 existing runtime tests ignored |
| Native production-state replay | Four option regressions plus the exact pinned migration rehearsal passed; explicit pre/post hooks and one multi-block migration step |
| Runtime local tests | 277 passed, including the two local tests caught by the broad remote-name filter and four option regressions; three existing ignores |
| Common / storage / generators | Common 138; parity-db 33 plus two interrupted-log recovery tests; FRAME four behavioral regressions; Wasm builder 14 |
| Order book / vesting / consensus helper / node | 24 order-book, 10 Ceres, two BABE helper and three node bootstrap tests |
| Exact-Wasm fork | Compatibility, upgrade lifecycle, equivocation, fee policy, paid migrations, retired lending, payout batches, bridge quotas and cleanup passed |
| Independent rejection check | 22 rejected calls / 52 raw writes; only three System accounting fields, no raw account, fee, reward, claim or event writes; node-style rollback preserved |
| Security audits | Eleven Cargo locks: zero vulnerabilities and RustSec warnings; six npm graphs: zero findings |
| JavaScript native interfaces | Canonical executor/crypto provenance and all 32 installed assets matched; installed interface, schema and crypto regressions passed |

Native replay downloaded the complete **750,029-key EthBridge prefix**, inspected 196,473 account-history entries, and preserved the audited bridge bounds. The scoped snapshot is 243,895,814 bytes with SHA-256 `139c89000936e2497094703b3501f77687ab03d1831f551d3cbc188829478a09`. Reproducible snapshots and public RPC caches are Git-ignored; their hashes remain in evidence.

The Wasm comparison checked 81 pallets, 406 calls, 443 events, 975 errors, 491 storage entries, 157 constants, seven signed extensions and 10,405 nested type pairs. Existing encoding/API contracts match; only the intended `System.Version` constant changes. The compiler removed optional linker global `__data_end`; reviewed hosts consume `__heap_base`, which remains required and checked. All callable imports/exports and memory limits pass, and the exact candidate executed successfully.

## Fixes made during verification

- Updated `uint` precisely from 0.10.0 to 0.10.2 and removed obsolete subxt lint declarations. Refreshed the root lock audit without unrelated package changes.
- Fixed FRAME `CloneNoBound` generation, including generic bounds, observable clone behavior, helper hygiene and empty enums; retained tuple predicate ordering and short-circuit behavior.
- Modernized constants, imports, test assertions and arithmetic predicates while preserving zero-divisor failures and storage behavior. Parity-db file creation explicitly preserves existing contents.
- Corrected macOS LLVM loader setup in the Clippy runner and supported both current and legacy Cargo timestamp layouts in the Wasm builder.
- Advanced the candidate spec version and strengthened native block/trie-version pinning, production-feature checks and full-source evidence binding.
- Corrected rehearsal lifecycle: finalize the timestamp-only upgrade block while it is `OnlyInherents`, verify migration completion, then require `AllExtrinsics` in the next block before signed cases. Model node rollback on validity rejection, separately inspect raw rejected writes, and reject stale or mismatched evidence.

The SDK error boundaries retain four narrowly documented `expect(result_large_err)` annotations: three test callback sites and the fork-off CLI boundary. No Clippy group was disabled. Cargo still emits dependency-manifest notices (such as unused patches and deprecated lint-key spelling); these are distinct from compiler warnings and security advisories.

## Scope and remaining operational limits

The native lane uses production chain features plus `common/test` instrumentation required by the chain-spec development dependency. It verifies retained migration state and explicit pre/post hooks, not whole-chain `try_state` or bit-identical production execution. The exact default-production Wasm fork supplies production-runtime behavior evidence. Its synthetic fixtures and mocked outer signatures are disclosed in the rehearsal reports; these are local rehearsals, not public transaction submissions or consensus proofs.

Production already has the reward remap/publication and BABE repair markers. Live replay therefore checks their idempotent paths; local native/synthetic fixtures cover active historical branches.

**Bridge readiness correction:** the earlier scanner misread the numeric storage-size RPC result as an optional value. The corrected [bridge follow-up](/Users/takemiyamakoto/dev/sora2-network/misc/runtime_upgrade/bridge_activation_readiness_2026-10-25.md) finds 12 stored pending calls and 2,049 hash-only operations and recovers all 2,061 preimages. The complete global stored-call inventory contains 1,357 entries; all observed payloads are at most 278 bytes. Operator readiness remains **unconfirmed**. No activation or backlog cleanup was performed.

Existing ignored tests remain unchanged: `tests::liquidity_proxy::chameleon_pool_swaps`, `tests::liquidity_proxy::chameleon_pool_swaps_burn_kxor`, `tests::xor_fee::reminting_for_sora_parliament_works`. The bridge-specific remote helper was not separately replayed; the full pinned migration lane performs complete bridge bounds/index checks before and after the upgrade. Historical release logs retain their original whitespace; current source passes the scoped diff check.

## Evidence and reproduction

- [Machine-readable final summary](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/final-verification-summary.json)
- [Clippy and production build](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/final-build-verification.json)
- [Native regression commands, counts and logs](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/native-regressions.json)
- [Pinned native migration replay](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/native-remote-execution.json)
- [Exact-Wasm rehearsal summary](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/rehearsal-summary.json)
- [Independent rejected-admission verification](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/rejected-admission-check.json)
- [Cargo audits](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/cargo-audit-2026-10-25.json) and [npm audits](../../runtime-rehearsal-dependency-migrations-2026-10-25/validation/npm-security-refresh/summary.json)
- [Rehearsal instructions and limitations](../../runtime-rehearsal-dependency-migrations-2026-10-25/README.md)

Run `housekeeping/clippy.sh all` for the full lint gate. The evidence records exact commands, LLVM environment, toolchain and immutable input hashes. Native replay requires its final fingerprint and a fresh scoped snapshot; the Wasm runner requires the candidate plus passed build evidence. Earlier failed/invalid-lifecycle attempts are retained and explicitly superseded by run `20261024T174836Z-fa72028d`.

The [2026-10-24 dependency migration report](dependency_audit_2026-10-24.md) remains a historical snapshot, including its Docker restart/connectivity and cross-platform native-build evidence; this report records the additional current-source gates.
