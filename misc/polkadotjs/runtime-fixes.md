# Runtime payout and fee fixes

## Atomic staking payouts

`runtime/src/xor_fee_impls.rs` implements `pallet_staking::AdditionalPayout`.
The vendored staking pallet calls it inside a storage transaction after claim
and exposure validation. The existing call indices and storage layouts remain
unchanged. Both payout entry points reserve the hook's maximum page weight;
the returned weight includes the current page's hook cost regardless of native
reward size. An error rolls back claim markers, token issuance, token balances,
and reward events.

`runtime/src/lib.rs` configures this hook and makes the older transaction
extension's payout hook a no-op, preventing duplicate minting. It leaves native
`EraPayout = ()` intact. Tests exercise legacy and paged payouts, self-only
validators, direct dispatch, all three utility batch modes, multisig, failed
batch rollback, deposit fault injection with retry, duplicate claims, fee
settlement without duplicate payment, and unchanged XOR issuance.

The weight bound conservatively composes the SDK staking payout bound with the
existing token `force_mint` benchmark for each recipient, plus destination
reads and event overhead. It counts all exposure entries, including skipped
payees and zero rewards. At the configured maximum of 256 nominators, the
combined declaration fits the runtime's normal-class weight limit. This is a
composed conservative bound, not a new hardware benchmark run.

The fork is pinned to SDK `polkadot-stable2603-3` / commit
`e3737178ec726cffe506c907263aaaa417893fd0`. Its complete 213-test pallet suite
passes, including two new hook-specific regressions.

## Token fee accounting (`wip`)

`LiquidityInfo::PaidInAsset` carries the asset ID and paid amount without
creating a native negative imbalance. Settlement refunds and pays referrals in
that asset only. Native fee behavior remains on the original imbalance path.

`BurntForFee` retains its gross-bucket layout. The new
`BurntForFeeReferrerPaid` storage records the exact referral amount already
reissued, including per-transaction rounding. Reminting subtracts this value
and uses the remaining fee weights for the VAL allocation. A legacy bucket
without that record is initialized from the old aggregate ratio before a new
payment is added. Exact historical per-transaction rounding cannot be recovered
from old aggregate storage; this limitation applies only to pre-upgrade pending
`wip` buckets. New payments are tracked exactly.

Tests verify unchanged native issuance, a full token-fee refund, repeated small
referrals, legacy-bucket initialization, exact fee-token issuance after remint,
and successful and failed conversion paths. These tests do not assert that
mainnet runtime 130 enables `wip`.

## Routed buyback weight

The runtime's `remint_val_swap` adapter uses the same route/filter for the
LiquidityProxy weight estimate and execution. The estimate is retained on
failure; successful execution contributes any weight exceeding it. This removes
the former unconditional XYK-only charge for a routed swap. A regression compares
accounted weight with the concrete router estimate and actual successful swap,
and verifies failure retains the estimate. It inherits LiquidityProxy's own
failure-weight estimation contract.

## Build compatibility and release scope

The WebAssembly check also required adding `use sp_std::vec::Vec` to the existing
Apollo module changes. Other pending work in this checkout was preserved.

These are source fixes and review artifacts. No runtime upgrade, chain-state
migration, hosted frontend deployment, or historical reward/issuance adjustment
was submitted. Normal release packaging and deployment remain separate from
this code-fix task.
