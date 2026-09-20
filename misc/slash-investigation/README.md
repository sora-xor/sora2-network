# SORA validator slash investigation — 2026-09-12

The recurring penalties are automatic `im-online:offlin` liveness reports. The chain evidence establishes the trigger, but does not establish malicious behavior or explain why these validators failed to get blocks or heartbeats included. Subsequent runtime regressions demonstrated additional defects in session-boundary accounting, recovery, and deferred slashing; those are source findings, not proof that any particular defect caused every captured incident. The affected validator identities still need to be matched to the operator's reviewed nodes.

## Additional production-code review

The initial 11-test result below describes an earlier policy revision. It is
superseded by the further review and remediation in
[additional-fixes.md](additional-fixes.md). The original passing checks did not
cover late re-enablement, real boundary-block hooks, or transaction admission.
The preserved `repro-*-before-fix.log` files record the failures that exposed those
gaps. Release artifacts prepared before these changes remain stale.

The final local candidate passed **37 focused tests, 187 runtime library tests
(four existing tests ignored), and 217 staking library tests**. Its release Wasm
build, metadata compatibility check, and local upgrade-initialization rehearsal
against pinned mainnet state also passed. Exact scope, hashes, and the remaining
large-workload limitation are in [lifecycle-validation.json](lifecycle-validation.json).
No upgrade has been submitted or deployed.

## Follow-up: repeated penalties after recovery

Source review confirms a feedback mechanism. In the pinned SDK, every current-era offence reaching Staking can disable its validator for the rest of the era. Im-online still treats the validator as offline unless it authors a block or supplies a heartbeat, while BABE rejects blocks from a disabled validator. Heartbeat sending and validation remain available to disabled validators; the evidence does not prove that valid, included heartbeats are being ignored. Disabled state normally clears at the next elected era, including when the elected validator identities are unchanged. This is not evidence of an unavoidable permanent disable state.

Upstream previously exempted offline offences from disabling in [Substrate PR #13493](https://github.com/paritytech/substrate/pull/13493). The later [SDK disabling redesign #2226](https://github.com/paritytech/polkadot-sdk/commit/988e30f102b155ab68d664d62ac5c73da171659a) removed that exemption. Those changes support the mechanism; they do not establish the date this network first became affected.

The current rule does not check whether consensus or finality actually stopped. Its positive penalty for six of 25 validators is sufficient by itself. Continuing consensus therefore does not exempt these reports under deployed spec 130.

The local policy fix routes ImOnline through `MajorityOfflineReports`. Offline penalties are allowed only if a **strict majority of the full active validator set is missing liveness in the same session**. Exactly half does not qualify: 12/25 and 12/24 are suppressed, while 13/25 and 13/24 qualify.

Validators disabled at any point during the session are excluded from both the offline numerator and the forwarded offender list. The bounded history survives re-enablement and expires at the next session boundary. They remain in the full-set denominator, so disabling cannot lower the threshold or inflate the outage count. Duplicate accounts and accounts outside the current validator set cannot establish a majority. The reporter rejects reports whose session or validator count does not match the current session, because disabled indices are meaningful only for that set. The partially observed session immediately after upgrade receives grace; complete history begins with the next session.

`SomeOffline`, `HeartbeatReceived`, heartbeat validation and authorship tracking remain available. ImOnline emits the original diagnostic list before the reporter filters it. Below-threshold reports do not enter Offences/Staking and therefore do not trigger new slashes or disabling. A qualifying majority still uses the existing offence bookkeeping and penalty formula: **13/25 and 13/24 produce the existing maximum 7% offline slash**. This is a threshold change, not a new penalty slope. A majority outage is a policy trigger, not proof of coordinated intent.

Qualifying offline reports retain financial penalties but no longer automatically disable validators. BABE/GRANDPA use a typed reporter that preserves standard Offences/Staking financial processing and explicitly preserves current-era equivocation disabling. Invalid proofs, duplicate reports, invulnerable exemptions, and historical-era handling retain their respective rules. This prevents an ending session's offline report from invalidating a returning boundary-block author.

Existing pending slashes are not cancelled by this configuration change. Existing disabled validators retain that state until normal era rotation; the fix does not indiscriminately clear disablements that may have come from equivocation. A fresh qualifying majority outage after a normal reset remains slashable. Runtime deployment is separate from the source change.

## Earlier validation of the local policy fix

All **11 focused tests passed**, and the default no-std WebAssembly target check passed. Formatting and whitespace checks also passed. The suite covers odd/even strict-majority boundaries, the six-of-25 incident, separate-session minority cohorts, disabled-validator exclusion, recovery after a qualifying majority incident, valid/invalid/duplicate heartbeat handling, and the unchanged BABE/GRANDPA validator/nominator financial paths. It also exercises defensive rejection/deduplication in the reporter. Exact final results and commands are recorded in `policy-fix-validation.json`.

The monetary tests exercise actual Offences/Staking handlers and deferred slash application. They do not manufacture or test cryptographic equivocation proofs, whose verification code is unchanged. No release Wasm was built in this follow-up. The previously prepared `runtime-upgrade-4.8.9` artifacts are marked as predating this source change and need rebuilding if this policy is to be included.

## Verified chain evidence

The primary finalized snapshot is block **27,622,680**, hash `0x8a681332bb3c8133bb02ce08c00c23489625915211ab4d6a99d7e2f72f591702`, captured at 2026-09-12 08:35 UTC (17:35 JST) from `wss://ws.mof.sora.org`. SORA runtime spec is **130**, active era **7637**, session **47127**.

The archive endpoint `wss://mof2.sora.org` matched SORA's genesis hash and the exact historical block hashes returned by the primary endpoint. At the completed session boundary:

| Block | Observation |
| --- | --- |
| 27,622,328, session 47,126 | 25 validators; 19 had authored a total of 472 recorded blocks; zero received heartbeats. The six addresses below had zero recorded blocks. The disabled-validator list was empty. |
| 27,622,329 | `imOnline.SomeOffline` named those six. Each received `staking.SlashReported` with fraction 25,200,000 parts per billion (2.52%), slash era 7637. `offences.Offence` identified `im-online:offlin` with session 47126. They were then disabled. No `staking.Slashed` debit event occurred in this block. |

Their missing authorship in that completed session was therefore not caused by already being disabled on chain. This also demonstrates that the pending queue is not merely a frontend display issue.

The retained offence index contains **1,066 validator/session offline reports** across **163 consecutive completed sessions, 46964–47126**. Every one of these six validators appears in every sampled session:

| Validator stash | Offline reports in sample |
| --- | ---: |
| `cnT5yU2sVMqYrPa3E2qLA8njJ8NuCJtqDemJRirFGCgDyP8x8` | 163 |
| `cnUJ6AJHd6CiKy3cSszgJy3yzvCKnUiM8UZzacLzXR8ZDkDb5` | 163 |
| `cnVRZ4uNBcSq9qeoYeoSu52FTjBGshoMf7gJesgfs7fxH3Nnt` | 163 |
| `cnWA4wz9H22gcevynWG6ysQuSfCw4i1jnneigKNX6h5vFZ5SA` | 163 |
| `cnWjWRxYHVg3wekb4ALe8z3rjiGKMZ5Cc9se5Q5sRkdqBXbee` | 163 |
| `cnWycFmujoB6XvBrBCK5EpLyh3z18BoUur9i9LrsbCKiZACeZ` | 163 |

Three other validator stashes appear earlier in the sample. Their identities and counts are in `summary.json`.

## Pending amounts and timing

All values below are queue face amounts at the pinned snapshot, using 18 decimal places for XOR. They are not a total of historical deductions or a guarantee of future realized debits.

| Scope | Amount/count |
| --- | ---: |
| Pending records | 193 |
| Validator stashes / distinct nominators | 9 / 41 |
| Total pending own + nominator amounts | 147.267377810983647148 XOR |
| Validator own amounts | 7.825086460872880261 XOR |
| Nominator amounts | 139.442291350110766887 XOR |
| Execution eras | 7638–7665 |
| Earliest execution batch, era 7638 | 8 entries, 12.954682455947467503 XOR |

**Era 7638 is the next era after the snapshot.** The UI showed roughly four hours remaining in era 7637. Era progression is the authoritative deadline; this is not a fixed wall-clock guarantee. Re-query current state before using these values.

For the six recurring validators above, all pending `own` amounts are zero; their pending rows affect nominator exposure. This does not establish ownership of the nominator accounts. The queue's separate reporter `payout` field is not an additional slash and is excluded from the totals.

The 1,066 reports do not imply 1,066 additive full deductions. Staking tracks the highest slash fraction per validator/era. If the fraction increases within an era, an incremental amount can be queued. In era 7616, the offline cohort increased from eight to nine; the extra eight incremental entries in execution era 7644 are consistent with that behavior, not evidence of duplicate full charging.

## Why apparently healthy nodes can receive these reports

The inspected runtime routes `ImOnline → Offences → Staking`. The liveness condition is an **included im-online heartbeat OR at least one recorded authored block during the session**. A process being up, syncing, having peers, or participating in GRANDPA finality does not by itself satisfy this condition.

For six offline validators among 25, the configured rule gives:

`min(3 × (6 − (floor(25 / 10) + 1)) / 25, 1) × 7% = 2.52%`.

This agrees with the actual archived events. The checked code is in `runtime/src/lib.rs` (ImOnline/Offences configuration) and the pinned SDK's `substrate/frame/im-online/src/lib.rs` (`is_online_aux`, `on_before_session_ending`, `slash_fraction`). The SDK revision resolved locally is `e373717` from `polkadot-stable2603-3`. Local source review alone is not proof of byte-for-byte deployed runtime equivalence; the archived event/state evidence independently confirms this instance of the behavior.

All 25 queued im-online keys match the current key list in validator order, and no duplicate queued BABE, GRANDPA, im-online or BEEFY keys were found. This checks on-chain consistency only. It does not establish that the reviewed node holds or uses those keys.

The next operator checks are:

1. Match these stash addresses and active public session keys to the exact nodes reviewed. Verify local key possession for BABE and im-online, not just a GRANDPA identity. Do not rotate keys as a diagnostic step.
2. Check validator role and whether offchain workers are enabled and running. `node/src/service.rs:747` only starts the worker runner when `config.offchain_worker.enabled` is true.
3. Inspect BABE authoring and im-online logs around session 47126 (ending **2026-09-12 07:51 UTC / 16:51 JST**) for missing keys, signing/authoring failures, rejected heartbeat submissions, stale sessions, lag, or network issues. Match timestamps to chain inclusion rather than only successful local submission.
4. Compare the claimed evidence of correct operation with this session's actual absence of blocks and heartbeats. That comparison is necessary to distinguish node configuration/availability problems from a liveness-detection defect.

## Cancellation path for entries cleared by review

Mainnet's defer duration is 27 full eras, and the current storage format keys `staking.unappliedSlashes` by **execution era**: offence era + 28. Polkadot.js displays an adjusted offence-era label as well as the execution era.

For penalties that the review concludes should be cancelled, the configured administrative path is `staking.cancelDeferredSlash(executionEra, sortedUniqueIndices)`, dispatched by Root or at least three quarters of Council. It removes the selected pending entries; it does not refund deductions already executed and does not repair the cause of subsequent reports.

`pending-index-inventory.json` contains raw execution-era keys, entry indices, validators and amounts at the snapshot for review. Inclusion in this inventory is not a recommendation to cancel every entry. **Re-query and match validator/amount data immediately before preparing a cancellation**, because previous cancellations change indices and era transitions execute batches. No cancellation calls were encoded, signed or submitted.

## Evidence files and reproduction

- `snapshot.json`: pinned finalized state and runtime constants.
- `history.json`: stored offline report IDs and details, plus the primary endpoint's historical-pruning errors.
- `archive-evidence.json`: successful archive state and events before/after the session boundary, with genesis/hash comparisons. The primary node's pruning errors do not mean these blocks are absent from the chain.
- `summary.json`: independent exact totals, offender cohorts and key-consistency checks.
- `pending-index-inventory.json`: 193 pending entries with raw storage indices at the snapshot.
- `capture-snapshot.ts`, `capture-history.ts`: read-only collectors. Run snapshot first, then history promptly, with `tsx`, `@polkadot/api` 16.5.6 and `@sora-substrate/type-definitions` 1.27.7 installed in an isolated scratch directory containing copies of the scripts. Set `SLASH_EVIDENCE_DIR` to a new directory to avoid replacing this evidence. The primary endpoint may prune the boundary state; archived boundary results were captured separately.

The pre-existing runtime and frontend changes in the checkout were preserved. The initial investigation added evidence only; the follow-up adds the ImOnline strict-majority policy and regression coverage. No chain state was changed and no transactions were submitted.
