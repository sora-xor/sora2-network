# SORA 4.8.12 runtime upgrade

Runtime spec **134**, transaction version **131**, closes the identified fee
bypasses while preserving free successful order cancellations and authenticated
bridge protocol operations. See [FEE_POLICY.md](FEE_POLICY.md) for the complete
policy and activation prerequisites.

BABE and GRANDPA reports, including the legacy `report_equivocation_unsigned`
methods, require signed funded payers and retain fees on success and failure.
Bare reports are rejected. Kensetsu accrue/liquidate and Apollo liquidation also
require signed funded keepers. Empty cancellation batches, failed/replayed
cancellations, unauthorized `rewards.add_umi_nft_receivers`, and repeated signed
requested-preimage provision retain fees. Real successful cancellations and the
first useful requested-preimage provision retain their refunds.

Registered bridge peers need **zero XOR** for authenticated allowed protocol
operations, including accepted operations whose local application fails.
Invalid proofs and replay are rejected before execution. Generic/substrate
inbound channels retain verified fresh bare submissions and also support signed
submissions. Peer membership does not exempt unrelated calls or arbitrary
wrappers. No incoming fee escrow or bond is introduced.

Successful Iroha migration retains its refund; failed settlement is paid. A
zero-XOR claimant can use a voluntary claimant-bound sponsor grant with capped
fees, budget, expiry and at most three attempts. No automatic sponsor is
configured. New sponsorship calls/storage/events/errors are append-only;
existing migration call encodings and storage remain compatible.

The candidate includes the preceding 4.8.11 staking reward repair. From the
captured spec 132 baseline, its bounded reward publication migration also runs.
Claims and ledgers must remain intact, and actual staking payouts remain VAL.

Wasm SHA-256: `9cdc615875e0664304c50bfc09350660388e4c015543fbb4d827995af9ad9037` (3,086,396 bytes).
Source overlay tree: `a16e9e1ac195575c6008c69434999b2587b81d01`. The 11 affected native
suites passed **1,140 tests**, with zero failures and three existing ignored
tests; another **430** maintenance/common/inbound bridge tests passed. All
**28** new runtime fee-policy regressions passed. The exact packaged Wasm passed
report-fee, broader fee-policy and metadata/host-interface compatibility checks.
[VALIDATION.md](VALIDATION.md) describes the checks and their limits.

The package contains six unsigned governance review calls. Verify the final ZIP
companion SHA-256 and extracted `SHA256SUMS`, then follow
[GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md). Finalized preflight block **27,907,565** still has
**4.8.11** in the external queue; `council-settings.json` records `activationReady: false`. The guarded
route cannot replace that proposal. Resolve the queue normally and refresh the
baseline before activation. If 4.8.11 enacts, repeat metadata compatibility and
exact-Wasm rehearsal against its deployed runtime.

Preparation does not install/fund keeper or reporter accounts, configure
relayers, attest operator rollout, sign transactions, or submit governance
calls. Operator readiness and fresh finalized-state checks are required before
activation.
