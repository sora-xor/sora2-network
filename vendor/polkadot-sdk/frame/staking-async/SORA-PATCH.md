# SORA Nexus asynchronous staking fork

This is the actual `pallet-staking-async` 0.16.0 implementation from Polkadot SDK
`polkadot-stable2609`, commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`, at
`substrate/frame/staking-async`. Its asynchronous election, session-report,
paged offence processing, era rotation, and pruning state machines are retained.
It is not an alias or rename of the older synchronous pallet.

`Cargo.toml.orig` is the original manifest. `Cargo.toml` materializes upstream
workspace package fields, dependency definitions, and lints; SDK dependencies
remain pinned to the same release. Relay-client interfaces come from the exact
upstream release dependency. The old synchronous fork remains available for
historical migration support.

## Local policies

- `AdditionalPayout` runs transactionally after claim/exposure validation. Hook
  failure rolls back claim, ledger writes, hook writes, and events. Both public
  payout calls reserve the maximum page hook bound; a failed hook retains that
  bound, and a successful claim refunds to the observed page size.
- Replacement rewards preserve the published `EraPayout` amounts and skip native
  inflation caps and remainder issuance. Their hook follows `PayoutStarted` and
  owns all asset settlement. Native minting, reward pots, incentive transfer,
  and `Staked` ledger compounding are bypassed for these claims. Replacement
  configuration requires `DisableMinting = false`.
- `SlashDeferOffset` defaults to zero for upstream behavior. SORA Nexus uses one
  extra era when `SlashDeferDuration` is nonzero, preserving its historical
  `offence + duration + 1` deadline. Same-era withdrawal waits for queued slashes;
  reports reaching their deadline apply during processing with the original
  offence era. An earliest-era cursor keeps overdue async queues executable
  across era transitions. Immediate slashing stays immediate. New unbond requests
  retain the historical planned-era deadline; withdrawal uses the active era.
- Historical ledger claim encoding is retained in `legacy_claimed_rewards`,
  and claims listed there cannot be paid again. Legacy exposure reads fall back
  to their historical storage. `EraInfo` remains a read facade for the VAL hook.
- Historical pre-cutover offences and unapplied slash vectors keep their original
  encoding, reporter lists, index cancellation semantics, and span/max policy in
  `legacy.rs`. Native asynchronous queues use `AsyncUnappliedSlashes` to avoid
  interpreting old vectors as the new keyed records. Historical storage has
  explicit pallet metadata entries with its original names, keys, and encoding.
  The exposure-page bound delegates metadata to the historical exposure type,
  retaining its named fields for clients as well as its original encoding.
  Frozen vector cancellation and retained span cleanup preserve their original
  validation errors and reserve the additional decode/database costs.
- Existing call indices 14 (`set_invulnerables`), 17 (`cancel_deferred_slash` with
  index vectors), 22 (seven-field `set_staking_configs`), and 33 (`manual_slash`)
  remain available. New async cancellation, maximum commission, and extended
  configuration use indices 36, 37, and 38 respectively.
- Original event and error SCALE indices are explicit. Historical `Unbonded`
  retains its two original fields; unlock eras remain available in ledger storage.
  New async events and errors are appended after the historical variants.
  Genesis accepts the original four-field staker tuple and minimum validator
  count. As in the previous synchronous fork, genesis ignores the legacy
  controller tuple element and bonds from the stash; existing on-chain
  controller relationships remain untouched.
- `initialize_local_genesis` seeds exposure from the local session chain's initial
  election without planning another era. Local consensus integration lives in
  the runtime bridge and uses the upstream session-report interface.
- Currency inspection preserves the deployed synchronous pallet's nonzero-hold
  precedence over residual legacy locks (source revision
  `36c466398e3f2760e8ab44fdb89de796f5b0dd99`). A consistent zero-stake ledger may
  retain a legacy lock without turning it into stake. Activation does not remove
  these locks. Explicit `migrate_currency` counts existing holds when checking
  capacity and establishes the full hold before removing a positive old lock.
  Unlike the deployed underfunded fallback, it never reduces ledger liabilities:
  ambiguous backing, missing bonds and insufficient capacity fail unchanged.
  Zero/absent lock amounts remain `AlreadyMigrated`, preventing repeated
  reference decrements through an interface that cannot distinguish those cases.
  The normal bonding path is unchanged; one extra database read is charged.

## New-era upstream slashing policy

The cutoff era and older offences use the retained historical span/max calculator,
reporter list, and reward formula. Later eras use the actual upstream asynchronous
policy: each validator/era tracks its maximum reported slash fraction, each page
charges the incremental fraction against that validator's exposure, and nominator
liabilities are no longer pooled through the old cross-validator span maximum.
The new pipeline retains the first reporter and pays the configured reward
fraction of the incremental slash, instead of the old span reward-halving formula.
This boundary is explicit; historical commitments are never recomputed through
the new formula. The replacement VAL payout hook continues to use ordinary era
reward points, and does not pay upstream native weighted incentive rewards.

## Verification and provenance

`src/tests/sora_customizations.rs` covers transactional replacement payout,
rollback/event order, full error weight, native-balance and ledger preservation,
legacy claims, delayed withdrawal protection, and late offence increments.
Upstream tests retain the upstream defaults and exercise the new async calls.
Build, test, Clippy, and production snapshot rehearsal evidence is maintained by
this repository's runtime migration verification workflow. Source presence alone
is not a claim that those gates have passed.

`stable2609-customizations.patch` and `stable2609-customizations.json` record the
complete source delta against this exact upstream commit and its replay check.
