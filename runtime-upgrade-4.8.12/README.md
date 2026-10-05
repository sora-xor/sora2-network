# SORA 4.8.12 runtime upgrade

Runtime spec **134**, transaction version **131**, closes transaction-fee bypasses
while preserving useful order-cancellation refunds and zero-XOR bridge operation.

**Legacy Iroha migration requires the claimant to pay XOR, including successful
VAL claims.** Insufficient XOR rejects admission before execution. Failed
settlement retains the fee and restores the claim. All proposed sponsorship
calls, storage, events, errors and fee-payer hooks are removed. Legacy claim
records, ownership proofs, referrals and pending multisig behavior remain intact.

BABE/GRANDPA reports and Kensetsu/Apollo maintenance require signed funded payers.
Successful useful order cancellations and first requested-preimage provision
retain refunds; failed/empty/replayed cancellations, unauthorized Rewards calls
and requested-preimage replay pay. See [FEE_POLICY.md](FEE_POLICY.md).

Registered bridge peers require **zero XOR** for authenticated protocol work,
including accepted local failures. Invalid/replayed requests reject before
execution. Successful outgoing approvals retain their full validation weight.
New bridge proposals have a fair per-proposer quota within the shared 128-operation
limit. Current quorum can cancel abandoned proposals at full capacity without
executing their payload or changing consumed incoming state. The preceding VAL
staking repair remains included.

Final Wasm SHA-256: `98f152040b1f084b53f7c2e024c0357c03ca62a89c4b2fd26546a0e446da55e4` (3,086,444 bytes).
Source overlay tree: `086a0444ac493f5e60896051fa4dc1fbb9f8e078`; reviewed base: `bb38216396835fae45de9c38104e4927a96eb36c`.
Fresh native validation: **1,151 passed**, zero failed,
**3 existing ignored**, including **32**
runtime fee-policy regressions. Five paid-migration cases and the bridge
approval-weight case are required by exact name.

The exact Wasm verifies paid VAL delivery, paid replay, failed-settlement rollback
and zero-XOR rejection, in addition to the existing report, bridge and cancellation
checks. Deployed-baseline compatibility remains strict. Comparisons to the prior
unreleased candidate permit only nine explicitly enumerated sponsorship ABI
removals. [VALIDATION.md](VALIDATION.md) discloses fixtures, limitations and historical
results; earlier execution is never relabeled as fresh validation.

All three stage/test chain-spec blobs are rebuilt from this source. Only `:code`
changes; all other JSON values and bytes remain intact. Feature-runtime binaries
and metadata are not duplicated in this package; workspace verification checks
the actual embedded blobs.

Six unsigned governance review calls match this Wasm and finalized preflight
block **27,910,230**. Verify the ZIP companion checksum and extracted
`SHA256SUMS`, then follow [GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md). The guarded
route preserves an occupied external queue. Refresh baseline/state/operator
readiness before signing. Preparation does not submit or enact transactions.

Fresh feature-runtime regeneration gate: **passed**.
Fresh three-profile Clippy gate: **passed**.
