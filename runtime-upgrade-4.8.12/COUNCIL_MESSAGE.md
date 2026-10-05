Council, please review **SORA 4.8.12**, runtime spec **134**, transaction version **131**.

This upgrade closes the identified fee bypasses. BABE/GRANDPA reports and Kensetsu/Apollo maintenance require signed funded payers. Successful order cancellations remain free; failed, empty and replayed cancellations pay. Unauthorized Rewards updates and requested-preimage replay also pay.

**Legacy Iroha migration requires the claimant to pay XOR, including successful VAL claims.** Insufficient XOR rejects the transaction before execution. Failed settlement retains the fee and rolls back the claim. The proposed sponsorship machinery has been removed; existing claim records and ownership checks remain intact.

**Bridge peers do not need XOR.** Authenticated protocol calls remain free, including accepted local failures. Invalid proofs and replays are rejected before execution; unrelated peer calls remain paid. Successful outgoing approvals retain their full validation weight. Each proposer has a fair share of the 128 new pending multisig slots (32 with four members), and current peers can cancel abandoned proposals by quorum at full capacity without executing their payloads. Existing incoming failure/consumed-hash behavior is preserved.

Fresh validation passed **1,151 native tests**, with 3 existing ignored tests, including five paid-migration regressions. The exact Wasm also verifies paid VAL delivery, paid replay, failed-settlement rollback and zero-XOR rejection, alongside the bridge, report-fee and cancellation checks. Staging/test chain-spec runtimes were regenerated from the same source. Evidence limitations and historical results are disclosed in [VALIDATION.md](VALIDATION.md).

Wasm SHA-256 (3,086,444 bytes):
`98f152040b1f084b53f7c2e024c0357c03ca62a89c4b2fd26546a0e446da55e4`

`system.setCode` proposal hash (3,086,450 bytes):
`0x0688cda5b9ff2d440fa06b43cced3597a140dbecc3858d392208d53042e05a3d`

At finalized block **27,910,230**, **4.8.11 remains queued**. Resolve that proposal normally, confirm funded keeper/reporter readiness, and refresh the baseline before activation. The guarded council call preserves the queue. If 4.8.11 enacts, repeat the exact-Wasm checks against its deployed runtime. Bridge peers remain exempt from XOR funding.

Verify the ZIP checksum and follow [GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md). The package includes six unsigned governance review calls and the preceding staking reward repair. **No governance transaction has been submitted.**
