# SORA runtime 4.8.9 — council proposal

Approve the upgrade of SORA mainnet from runtime spec 130 to spec 131,
transaction version 131. This complete candidate includes the latest payout,
bridge, liquidation, validator liveness and deferred-slashing fixes.

## Decision and exact artifact

Register the preimage below, propose it through Council as
`Democracy.external_propose_majority`, then use TechnicalCommittee fast-track
with 1,800 voting blocks and zero enactment delay. The public referendum must
still pass. These are unsigned preparation artifacts; no proposal has been
submitted by this task.

| Item | Value |
| --- | --- |
| Runtime | `sora-substrate`, package `4.8.9`, spec `131`, transaction `131` |
| Release commit | `823a5b9fde8486dd73aeab856c76bbab5408bf0f` |
| Wasm | `framenode-runtime-4.8.9.compact.compressed.wasm` |
| Wasm length | 3,055,506 bytes |
| Wasm SHA-256 | `db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447` |
| Wasm Blake2-256 | `0xf062ed07861255f5ec443930de9c5066d1e4133036126d48462c8e233a75f27e` |
| `System.set_code` preimage hash | `0x6de23331a9c21a5ca06944b6cce28c0c0304c57ecd9b26af9fe2bb413b213b04` |
| `System.set_code` preimage length | 3,055,512 bytes |
| Council threshold | 4 of 8 minimum; threshold-5 alternative supplied |
| TechnicalCommittee threshold | 3 of 4 |
| Referendum voting period | 1,800 blocks, approximately 3 hours at 6-second blocks |
| Enactment delay | 0 blocks |

The Wasm hash and proposal hash refer to different bytes. The proposal hash is
Blake2-256 of the SCALE-encoded `System.set_code` call, including its call index
and encoded Wasm length. It is the hash used by Democracy.

## Resulting behavior

1. **VAL staking payouts:** direct, paged, utility-batched and multisig payouts
   execute the same atomic payout hook. Failure rolls back minting and claim
   markers. Native zero-reward eras continue to pay recorded VAL rewards, and
   the declared/returned weight includes that execution.
2. **Fee buyback accounting:** routed VAL buybacks account for the actual
   LiquidityProxy route's work. Token-fee fixes behind `wip` remain disabled in
   this mainnet feature set.
3. **Bridge:** bound request histories and approvals, protect incoming request
   identity/replay handling, improve failed-import recovery, and adapt deposit
   log ranges for supported oversized-response failures. The v3→v4 migration
   keeps the newest 2,048 entries in each account's noncanonical history index;
   canonical request records and statuses remain intact.
4. **Liquidation controls:** Apollo limits liquidation admission and improves
   health-factor validation; Kensetsu bounds accrue work per block.
5. **Validator liveness:** offline financial penalties require a strict majority
   of the full validator set within one session. Validators disabled at any
   time during that session do not count or receive repeat offline penalties.
   Offline financial penalties no longer disable recovering consensus authors;
   BABE/GRANDPA equivocation penalties and disabling remain enforced. The
   partially observed upgrade session receives offline-penalty grace.
6. **Session boundaries and deferred slashing:** session rotation precedes
   authorship accounting so the boundary block credits the incoming session's
   author. Unlocking uses the active era, deferred slash allocation uses the
   original offence era, and newly admitted overdue reports apply immediately.
7. **Equivocation weights:** initialize a bounded history/exposure cache and
   include exposure and deferred-queue work in admission weights. No weight is
   clamped merely to admit a transaction exceeding the block budget.

## Validation and compatibility

The release source is captured in a dedicated commit, Git bundle and full source
archive. Its source hashes match the build. All 209 runtime tests passed, with
four existing ignored tests. Verified supporting suites include 217 staking
pallet tests, 288 bridge tests and 37 focused liveness tests.

Metadata comparison preserves all 80 existing pallet indices, 405 calls,
481 existing storage entries and seven signed extensions. `Liveness` is added
at pallet index 120. Runtime and transaction versions increase to 131; existing
signed transactions may need to be recreated after enactment.

Both native archive-state rehearsals passed. The bridge history migration took
two steps and trimmed six account indexes while preserving canonical records
and replay mappings. Exact compiled-Wasm replay completed the upgrade, migration,
direct/paged/batched payouts, and duplicate-claim rejection. A separate Wasm
check verified liveness initialization against retained exposure metadata.
See `VALIDATION.md` for exact block references, final test results and limits.

## Operational limits

Unbonding remains 28 eras and slash defer remains 27 eras. This release does not
cancel existing slash queues, automatically drain historically stranded queues,
relock legitimately completed withdrawals, repair historical payout claims, or
adjust historical token issuance. Oversized synchronous equivocation workloads
remain non-admissible; missing/stale/legacy/over-limit exposure history uses a
conservative fallback. Weight allowances compose existing measurements and
conservative bounds; no new hardware calibration was performed.

The included Polkadot.js Apps patch needs a separate frontend release for VAL
amounts to appear on the staking payout page. The bridge source changes do not
establish the cause of every production RPC failure or repair the unavailable
external watchdog source.

This candidate supersedes the earlier payout/bridge-only proposal
`0xcf83851e197e1597ded8ac24f7b9e5141c3db641ba8a7f61b602b3767e4c1420`.
Use only the proposal hash and files in this complete package.
