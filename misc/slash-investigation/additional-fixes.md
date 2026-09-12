# Additional validator-lifecycle fixes

Status: local fixes, 37 focused runtime regressions, the full runtime/staking library suites, WebAssembly checking, and a release Wasm build have passed. The built candidate also passed a local upgrade-initialization rehearsal against pinned mainnet state. No governance transaction has been signed or submitted. The older release/governance bundle does not contain these changes.

## Reproduced defects and corrections

1. **False majority after re-enablement.** The initial majority adapter checked only the final disabled set. A validator forcibly inactive earlier in a session could be re-enabled just before reporting and turn 12 independently missing validators into a slashable 13/25. Bounded per-session disablement history now excludes that validator from both the numerator and penalties. Unknown history receives grace until a complete session starts.
2. **Wrong boundary-block author and lost liveness credit.** Authorship initialized before Session, resolving the incoming BABE index against the outgoing validator order and then erasing its old-session credit. Authorship now runs after Session, retaining pallet index 16 and the existing key encoding. A real runtime hook test checks reordered authorities, reward points, and liveness records.
3. **Recovery interaction at the boundary.** Correct old-session accounting can expose a returning boundary author to automatic offline disabling before BABE finalization. Offline reports now retain financial penalties without disabling. A typed equivocation reporter retains standard Offences bookkeeping and Staking penalties and explicitly preserves the existing current-era disabling rules. The real BABE finalization regression passes after this separation.
4. **Withdrawal before a queued penalty executes.** Planned CurrentEra advanced one session before the corresponding ActiveEra and allowed full withdrawal before its pending slash. Withdrawal now consolidates against ActiveEra. The staking regression reaches this state through real session rotation.
5. **Late admitted reports stranded in a consumed queue.** An offence first admitted after its scheduled execution era started was queued into an era that would never be visited again. Such reports now apply immediately against still-held stake using the original offence era.
6. **Wrong offence era during deferred application.** Queue keys are offence era + defer +1, but application subtracted only defer. Subtracting the full delay preserves proportional slash allocation between active and unlocking stake. Queue encoding and cancellation indices are unchanged.
7. **Equivocation transaction admission.** Actual BABE/GRANDPA signed and unsigned calls declared approximately 5.256 seconds against a Normal extrinsic limit of 1.459 seconds. All four failed the actual CheckWeight admission gate. A bounded exposure-work cache now counts every nomination row across all retained validators and eras. Weights include concurrent offenders, immediate late slashes, and live deferred-queue serialization, including queue growth inside batches. All four actual call types now pass CheckWeight for the captured workload. Missing/stale/legacy/over-limit cache states fail conservatively. Direct offence-handler tests alone would not establish transaction admissibility.

The pre-fix failures are preserved in `repro-majority-before-fix.log`, `repro-session-author-before-fix.log`, `repro-boundary-recovery-before-fix.log`, `repro-deferred-before-fix.log`, and `repro-equivocation-admission-before-fix.log`. Permanent regressions live in `runtime/src/tests/liveness.rs` and its child modules, with additional staking-package tests in the vendor directory.

## Scope and limits

These tests establish local source behavior. They do not establish exploitation or prove that every recorded mainnet slash had the same cause. Four-call admission fixtures isolate real dispatch-weight validation and use minimal structural proofs. A separate integration test supplies genuinely signed BABE/VRF evidence and archived membership proofs, passes outer Utility batch admission, then verifies real proof validation, penalty queues, and consensus disabling. Its election results are supplied by the test fixture; SDK trie generation and proof verification are real. The signature and historical ownership-verification implementations are unchanged.

Existing queued penalties are not cancelled. Historically stranded queue keys are not automatically collected. Legitimately completed withdrawals are not retroactively relocked, and late evidence can only collect stake still held. The full configured nomination workload can exceed a single block; truthful weight accounting must reject oversized synchronous work rather than undercharge it. General support at that size requires paginated financial slash processing.

The read-only finalized exposure snapshot found 29 retained eras, 749 paged metadata entries, no legacy exposure entries, and a maximum 113 total nomination rows in any retained era. Counts include repeated nominations to different validators; they are not counts of unique accounts. Exact block and timestamps are in `exposure-work-snapshot.json`.

## Validation

- Focused runtime lifecycle/admission suite: **37 passed**, no failures.
- Complete runtime library: **187 passed, 4 existing tests ignored**, no failures. The ignored tests are named in the final validation record.
- Complete staking library: **217 passed**, no failures or ignored tests; all four new staking regressions pass.
- Mainnet no-std WebAssembly check and release build: passed.
- Metadata compatibility: existing SCALE layouts preserved for 405 calls, 481 storage entries, and all seven signed extensions; no breaking changes found.
- Actual candidate Wasm initialized a local upgrade from spec 130 to 131 at pinned block 27,620,167; the seeded exposure cache matched an independent 752-entry scan, and incomplete-session grace was preserved. This stopped after initialization, with no extrinsics or finalization.
- New files are formatted and changed tracked files pass whitespace checks.

The new compressed candidate is `framenode-runtime-lifecycle-fixes.compact.compressed.wasm`, spec 131, 3,055,506 bytes, Blake2-256 `0xf062ed07861255f5ec443930de9c5066d1e4133036126d48462c8e233a75f27e`. It has not been deployed. The older `runtime-upgrade-4.8.9` proposal/preimage files must not be mistaken for this candidate.

`candidate-source.patch` and `candidate-new-source.tar.gz` record the source state, including pre-existing workspace changes. This is the full candidate context, not an isolated patch containing only this investigation. Final commands, logs, source hashes, and limitations are in `lifecycle-validation.json`.
