# Review: SORA runtime 4.8.10 to 4.8.11

Reviewed on 30 September 2026 (Japan time). No release-blocking runtime defect
was found in the reviewed changes. The council handoff has been strengthened
to reject an occupied external-proposal queue atomically at execution time.

## Scope and source

The baseline is tag `4.8.10`, commit
`51949ad854fc7c1a04a2c6a0f1d352b292caac35`. The reviewed checkout is branch
`4811`, commit `a05ecb8de476bcdf0212a1a646a1fbcac2f0b5c5`.

The existing candidate was built and tested from
`870df1dbe47507456faff6ab1975efc6493ce56d`. Its runtime build inputs match the
reviewed checkout. All 17 recorded source-file hashes match. Reuse preserves
the exact binary covered by the execution evidence; this handoff does not
claim a new build from the checkout HEAD. Original build provenance remains
in `validation/source-provenance.json`; the current comparison is recorded in
`validation/current-source-review.json`.

## Changes and assessment

| Change | Result and review |
| --- | --- |
| Runtime and package versions | Release 4.8.11, spec 132 to 133. Transaction version remains 131; the structural comparison preserves calls, events, storage layouts, signed extensions and runtime API versions. |
| Completed-era reward publication | Existing recorded VAL budgets become visible through `staking.erasValidatorReward`, allowing unmodified standard staking derives to discover rewards. |
| One-time migration | Updates only existing completed-era reward entries in the 84-era claim window. The migration is bounded and idempotent; it preserves claim markers, ledgers, exposure, reward points and preferences. It does not recover expired rewards. |
| Future era closure | `ValEraPayout` publishes the ending era's recorded VAL budget. Native reward capping and remainder issuance are disabled for this replacement reward policy. |
| Payout behavior | XOR remains staked and rewards remain VAL. Native payout/compounding is disabled; actual successful VAL payments also emit standard `staking.Rewarded` events after `PayoutStarted`. Transactional claim bookkeeping and rollback remain intact. |
| Node-only changes | Three test/staging chain-spec files and the BABE repair CLI return-type refactor differ from the original candidate source. These are not inputs to this runtime WASM. The archived native node build covers its recorded source, not a fresh HEAD node build. |
| Release artifact housekeeping | Later commits copy/remove 4.8.10 council artifacts. The commit titled `update xor staking` does not introduce a further runtime change. |

## Handoff issue corrected

The original council call used `democracy.externalProposeMajority` directly.
That call can overwrite a proposal queued after preparation. A preflight check
alone does not prevent replacement when the council motion eventually executes.

The preferred council call now wraps
`externalPropose(candidate)` followed by `externalProposeMajority(candidate)`
in `utility.batchAll`. The first call requires an empty queue; an occupied
queue makes the whole batch fail. Both calls accept the same at-least-half
council origin. The second call sets the intended simple-majority referendum
threshold. The runtime proposal preimage and its hash are unchanged by this
governance wrapper. The runbook identifies the older unguarded route as a
review-only alternative that would require an explicit supersession decision.

## Evidence and limitations

The original, hash-verified validation records 200 runtime and 218 staking
tests passing (four existing runtime tests ignored), native migration rehearsal,
and local execution of the exact candidate against pinned mainnet state.
The exact-WASM run published 83 completed-era budgets, discovered positive
pending rewards with unmodified upstream derives, paid VAL through an ordinary
staking call, preserved XOR stake, and rejected duplicate claims.

The current preparation verifies source equivalence, the package manifest,
exact embedded WASM/preimage bytes and unsigned governance encodings, and
refreshes finalized public chain preflight. The runtime suites were not rerun
and no fresh runtime build was necessary because their inputs are unchanged.
Full evidence and its boundaries are in `VALIDATION.md`.

Standard clients may still label the displayed amount XOR although payment is
VAL, and generic estimates can differ from actual payouts. The local rehearsal
uses mocked signatures and synthetic BABE headers; it does not prove network
consensus or real governance enactment. Recheck the live baseline, membership,
queue, preimage and motion state before signing. No transaction was signed or
submitted, and no production node was changed.

## Goal completion criteria

- Review the changes against 4.8.10 and account for later source changes.
- Verify that the delivered WASM is the exact validated candidate.
- Refresh mainnet preflight and verify council/technical committee call settings.
- Deliver the WASM, unsigned calls, council message, settings and integrity hashes.
