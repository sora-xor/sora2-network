Council, please review **SORA 4.8.12**, runtime spec **134**, transaction version **131**.

This upgrade closes the identified fee bypasses. BABE/GRANDPA reports require signed funded payers. Successful order cancellations remain free; failed, empty and replayed cancellations pay. Unauthorized Rewards updates and requested-preimage replay also pay.

**Kensetsu and Apollo enter repayment-only mode.** Kensetsu owners can repay and close existing positions to recover collateral; new CDPs, borrowing, collateral deposits and liquidation are disabled. Existing Kensetsu debt continues accruing interest under existing terms. Repayment or closure books accrued interest with the existing treasury accounting; no new borrowing is permitted. Apollo keeps repayment, withdrawal and earned-reward claims, while new lending and liquidation are disabled. Principal exits do not depend on reward funding or buyback liquidity; received Apollo protocol interest is reserved for separately authorized distribution. Both automatic maintenance workers are disabled.

**Legacy Iroha migration requires the claimant to pay XOR, including successful VAL claims.** Insufficient XOR rejects the transaction before execution. Failed settlement retains the fee and rolls back the claim. The proposed sponsorship machinery has been removed; existing claim records and ownership checks remain intact.

**Bridge peers do not need XOR.** Authenticated protocol calls remain free, including accepted local failures. Invalid proofs and replays are rejected before execution; unrelated peer calls remain paid. Successful outgoing approvals retain their full validation weight. Each proposer has a fair share of the 128 new pending multisig slots (32 with four members), and current peers can cancel abandoned proposals by quorum at full capacity without executing their payloads. Existing incoming failure/consumed-hash behavior is preserved.

Fresh validation passed **1,413 native tests**, with 3 existing ignored tests, including paid-migration and retired-lending regressions. The exact Wasm also verifies paid VAL delivery, paid replay, failed-settlement rollback and zero-XOR rejection, alongside the bridge, report-fee and cancellation checks. Staging/test chain-spec runtimes were regenerated from the same source. Evidence limitations and historical results are disclosed in [VALIDATION.md](VALIDATION.md).

Wasm SHA-256 (3,066,913 bytes):
`de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221`

`system.setCode` proposal hash (3,066,919 bytes):
`0xd853592afec19f9cf49dba93f470bc101261ec08f2aeb9ee8035210880dff313`

At finalized block **27,911,262**, **4.8.11 remains queued**. Resolve that proposal normally, confirm funded signed-reporter readiness and existing-position exit readiness, and refresh the baseline before activation. The guarded council call preserves the queue. If 4.8.11 enacts, repeat the exact-Wasm checks against its deployed runtime. Bridge peers remain exempt from XOR funding.

Verify the ZIP checksum and follow [GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md). The package includes six unsigned governance review calls and the preceding staking reward repair. **No governance transaction has been submitted.**
