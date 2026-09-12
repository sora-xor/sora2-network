# Validation of the complete 4.8.9 candidate

Release commit: `823a5b9fde8486dd73aeab856c76bbab5408bf0f`. Candidate SHA-256: `db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447`.
All runtime source files matched the captured release commit after validation.

| Check | Result | Evidence |
| --- | --- | --- |
| Optimized release Wasm | Passed | `validation/build.log` |
| Runtime plus try-runtime hooks | 209 passed, 0 failed, 4 existing ignored | `validation/runtime-tests.log` |
| Staking pallet | 217 passed, 0 failed | `validation/staking-tests.log` |
| Bridge pallet | 288 passed, 0 failed | `validation/bridge-tests.log` |
| Focused liveness | 37 passed, 0 failed | `validation/liveness-focused-tests.log` |
| Existing SCALE encodings | Compatible; comparator mutation checks passed | `validation/metadata-compatibility.json` |
| Native Executive migration | Passed, 2 migration steps | `validation/native-migration-rehearsal.log` |
| Native focused bridge migration | Passed, canonical state preserved | Same log |
| Exact compiled Wasm, fresh state | Upgrade, bridge migration, payouts and duplicate rejection passed | `validation/wasm-current-state-rehearsal.json` |
| Exact compiled Wasm, fresh liveness | Cache, exposure counts and initial-session grace passed | `validation/wasm-current-liveness.json` |
| Source commit and bundle | Source hashes match, bundle verifies | `validation/source-verification.json` |
| Wasm/preimage/governance bytes | All seven unsigned calls verified | `validation/package-check.json` |
| Collective action helper | 6 offline tests passed; live weight lookup passed | `validation/collective-helper-tests.log`, `validation/collective-actions-check.json` |

Fresh governance metadata and both fresh Wasm rehearsals use finalized block
27624441 (`0x4540fa40a592c0f7f5fb925bed05d8507bb77048b1fd8647ec5235665f78d7e8`), captured 2026-09-12T12:12:21.973Z.
Runtime 130 was still deployed. Its code matches the archived 4.8.8 runtime.
The liveness scan counted 774
overview rows, maximum 113
nomination edges and 28 validators
per retained era. All three payout routes paid the same
1193586644401646408 VAL base units (18 decimals) for the fresh sample.
The local block included the payout; repeating it returned `AlreadyClaimed`.

Native migration assertions used the retained immutable block 27620167 snapshot
(`0xedc2506edcb75c79c12d26a8c89a2473677a8aff67b3e05eb310573bdbf2a049`).
Both rehearsals ran against the complete latest source. Six account histories
were reduced from a maximum 30,720 entries to 2,048; canonical request/status keys
and replay mappings remained valid. Fresh Wasm independently confirmed storage
version 4 and cleared migration cursor after two local migration blocks.

The four existing ignored runtime tests are:

- `tests::liquidity_proxy::chameleon_pool_swaps`
- `tests::liquidity_proxy::chameleon_pool_swaps_burn_kxor`
- `tests::xor_fee::reminting_for_sora_parliament_works`
- `xor_fee_impls::tests::check_calls_from_bridge_peers_pays_no`

## Limits of the evidence

The native snapshot contains selected prefixes covering the configured migrations
and audited storage versions, not all pallets or child tries. Wasm execution
uses Chopsticks 1.5.1 with a local code overlay, mocked signatures, unresolved
imports allowed and offchain workers disabled. This exercises actual compiled
runtime behavior, but it is not a validator-consensus, host-import completeness,
or signed-governance rehearsal. Liveness-only runs stop after initialization;
the full payout runs finalize blocks and complete the bridge migration.

Supporting staking, bridge and focused liveness logs were retained from the
matching source state; their source/evidence hashes were verified. The combined
runtime suite, build, metadata comparison and all reported candidate Wasm/native
rehearsals were run for this complete package. No chain transaction was submitted.

## Repeating checks

From the extracted package directory, check the package and compare metadata
offline. The captured baseline metadata and runtime 130 are included:

```sh
python3 validation/verify-package.py
python3 validation/compare-metadata.py --output /tmp/sora-metadata-comparison.json
```

For native replay, extract `source.tar.gz` into a new empty source directory and
pass its absolute path to `bash validation/rehearse.sh /path/to/extracted-source`.
The script reuses the packaged snapshot, checks its exact block hash and runs
the two native tests. Its default LLVM paths match the original Mac; set
`LIBCLANG_PATH` and `LLVM_CONFIG_PATH` for a different build environment.

For exact Wasm replay, install the pinned dependencies and write new reports
outside the package so that the original evidence and checksums remain intact:

```sh
UPGRADE_JS_DEPS="$(mktemp -d)"
cp validation/rehearsal-package.json "$UPGRADE_JS_DEPS/package.json"
cp validation/rehearsal-package-lock.json "$UPGRADE_JS_DEPS/package-lock.json"
npm ci --ignore-scripts --prefix "$UPGRADE_JS_DEPS"
UPGRADE_REPLAY_BLOCK=0x4540fa40a592c0f7f5fb925bed05d8507bb77048b1fd8647ec5235665f78d7e8
NODE_PATH="$UPGRADE_JS_DEPS/node_modules" node validation/rehearse-runtime-upgrade.cjs \
  --wasm "$PWD/framenode-runtime-4.8.9.compact.compressed.wasm" \
  --endpoint https://mof2.sora.org --block "$UPGRADE_REPLAY_BLOCK" \
  --cache "$UPGRADE_JS_DEPS/read-cache.json.gz" \
  --output "$UPGRADE_JS_DEPS/payout-rehearsal.json"
node validation/rehearse-liveness-upgrade.cjs \
  --dependencies "$UPGRADE_JS_DEPS/package.json" \
  --wasm "$PWD/framenode-runtime-4.8.9.compact.compressed.wasm" \
  --endpoint https://mof2.sora.org --block "$UPGRADE_REPLAY_BLOCK" \
  --read-cache "$UPGRADE_JS_DEPS/read-cache.json.gz" \
  --output "$UPGRADE_JS_DEPS/liveness-rehearsal.json"
```

These Wasm replays require the pinned block to remain available from archive RPC.
Reports record the exact runtime hash, block hash and executor limits.
