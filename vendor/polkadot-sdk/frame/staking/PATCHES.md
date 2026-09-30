# Local staking changes

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2603-3`, commit
`e3737178ec726cffe506c907263aaaa417893fd0`, path `substrate/frame/staking`,
package `pallet-staking` version `46.0.0`. The upstream Apache-2.0 notices
are retained in the source files.

The repository root patches the SDK `pallet-staking` package to this directory.
`Cargo.toml` expands upstream workspace package metadata and dependencies;
all SDK dependencies retain the same pinned release tag and feature defaults.
The manifest also expands the Rust and Clippy lint tables inherited from that
SDK workspace, so vendoring preserves its lint policy without changing the
SORA workspace policy.
The complete local diff against that upstream directory is reproduced by applying
[`additional-payout.patch`](additional-payout.patch), followed by
[`deferred-slashing.patch`](deferred-slashing.patch), then
[`native-reward-compatibility.patch`](native-reward-compatibility.patch).
The first two patches preserve their existing reviewed changes; the compatibility
patch applies to their resulting sources and is checked against this directory.

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
