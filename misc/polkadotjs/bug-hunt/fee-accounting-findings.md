# Fee accounting findings and repairs

The two `wip` accounting defects are fixed in the runtime and xor-fee pallet.
See [the implementation notes](../runtime-fixes.md) and
[executed validation](../validation.md).

1. Fee-token withdrawal used to fabricate a native negative imbalance. Dropping
   it reduced XOR issuance without debiting an XOR account. The new
   `PaidInAsset` variant carries only the fee asset and amount; native issuance
   remains unchanged through withdrawal, settlement, and full refund.
2. Referred token fees used to mint the referral amount and later remint the
   entire gross fee again. Reminting now subtracts the exact already-paid
   referral amount, including per-transaction rounding. The existing gross
   storage layout remains compatible; pre-upgrade aggregate buckets use the
   documented legacy estimate when exact historical amounts are unavailable.

Actual FRAME integration tests now cover these repairs. The earlier proposed
standalone test patch and extracted arithmetic model were superseded by tests
in `runtime/src/tests/xor_fee.rs`.
