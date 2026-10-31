# SORA Nexus staking customizations

Current upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, package `pallet-staking` 50.0.0.

The current complete source delta is [`stable2609-customizations.patch`](stable2609-customizations.patch),
with a checked byte-for-byte replay and hashes in [`stable2609-customizations.json`](stable2609-customizations.json).
The three older patch files are retained as historical review layers against their
original stable2603 base, and should not be applied directly to stable2609.
The normalized manifest expands this release’s workspace metadata/dependencies/lints.

Stable2609 marks synchronous staking deprecated in favor of `pallet-staking-async`.
This upgrade preserves the existing synchronous state machine and economic policy.
Async staking requires its own explicit state and policy migration.

## Deferred slashing

Withdrawals consolidate unlocking chunks using `ActiveEra`, so planning the next
era cannot release funds before its deferred slashes run. Queue execution keys
remain `offence_era + SlashDeferDuration + 1`; application now subtracts that
entire delay to preserve the original offence era and proportional unlocking
allocation. A newly admitted report whose execution era has already started is
applied immediately using its original offence era, instead of being appended to
a queue that has already been consumed.

Storage encoding and cancellation indices are unchanged. This does not execute
historically stranded queue entries or extend the contractual unbonding period.
Evidence received after a completed, legitimate full withdrawal may have no
remaining bonded stake to collect.

```sh
cargo test --locked -p pallet-staking --lib deferred_slash_
```

Runtime policy, boundary-block authorship, and recovery regressions are in
`runtime/src/tests/liveness.rs` and its child modules.

## Additional payout hook

`Config::AdditionalPayout` defaults to `()`. Its `payout(stash, era, page)`
method runs in the staking payout helper after claim/exposure validation, in
the same storage transaction as claim bookkeeping and native rewards. A hook
error rolls back its writes and the claim, permitting a later retry. This
applies to direct and nested dispatch through both existing payout calls.
Hook implementations must not recursively dispatch staking payouts or mutate
staking exposure, reward points, preferences, or claim bookkeeping.

The hook supplies a conservative `weight(nominators)` bound, including failure
paths. Both call declarations reserve the maximum page bound. Successful calls
retain the actual page's hook bound even if native rewards are zero. Hook
failures retain the full native page bound plus the hook bound. Existing call
indexes and storage layouts are unchanged. Native reward behavior remains the
default; replacement-asset behavior is an explicit opt-in described below.

The mock hook is a no-op unless a test enables it. Two focused regressions
exercise failed-hook rollback and retry, and declared/refunded hook weight
when the native era reward is zero:

```sh
cargo test -p pallet-staking --lib additional_payout
```

Runtime-specific VAL accounting and wrapper-dispatch regressions live in
`runtime/src/xor_fee_impls.rs`.

## Replacement reward compatibility

`AdditionalPayout::pays_native_reward()` defaults to `true`, preserving upstream
native staking rewards. A hook returning `false` owns payment of the published
era budget in a replacement asset. The ordinary `ErasValidatorReward` storage,
claim validation and payout calls continue to expose that budget to clients.
The replacement hook must implement the actual asset payment and any standard
reward events required by those clients.

In replacement mode, native `make_payout` exits before looking up the payee,
depositing currency or increasing a `Staked` ledger. Era finalization preserves
the configured `EraPayout` amounts rather than applying `MaxStakedRewards`, and
does not issue its remainder in the native staking currency. Asset-specific
remainder policy belongs to the replacement implementation. Claims and hook
payments retain their existing storage transaction and weight accounting.
Replacement pages emit `PayoutStarted` before invoking the hook, so standard
reward events emitted by that hook follow their page marker. A failed hook
rolls back the marker event together with its claim. Native rewards keep their
existing event ordering and zero-reward-points behavior.

Three regressions cover direct and paged claims with a nonzero advertised reward,
unchanged native balances, issuance and `Staked` ledgers, successful hook/claim
bookkeeping and duplicate rejection; event ordering and rollback after hook
failure; and an era cap below 100% that neither reduces the published replacement
budget nor issues native remainder:

```sh
cargo test --locked -p pallet-staking --lib replacement_reward_
```
