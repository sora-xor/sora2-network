# Local staking changes

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2603-3`, commit
`e3737178ec726cffe506c907263aaaa417893fd0`, path `substrate/frame/staking`,
package `pallet-staking` version `46.0.0`. The upstream Apache-2.0 notices
are retained in the source files.

The repository root patches the SDK `pallet-staking` package to this directory.
`Cargo.toml` expands upstream workspace package metadata and dependencies;
all SDK dependencies retain the same pinned release tag and feature defaults.
The complete local diff against that upstream directory is reproduced by applying
[`additional-payout.patch`](additional-payout.patch), followed by
[`deferred-slashing.patch`](deferred-slashing.patch). Both patches were replayed
against the pinned upstream files and compared byte for byte with this directory.

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
indexes, storage layouts, and native reward calculations are unchanged.

The mock hook is a no-op unless a test enables it. Two focused regressions
exercise failed-hook rollback and retry, and declared/refunded hook weight
when the native era reward is zero:

```sh
cargo test -p pallet-staking --lib additional_payout
```

Runtime-specific VAL accounting and wrapper-dispatch regressions live in
`runtime/src/xor_fee_impls.rs`.
