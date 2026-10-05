Council, please review **SORA 4.8.12**, runtime spec **134**, transaction version **131**.

This upgrade closes the identified fee bypasses. BABE/GRANDPA reports and Kensetsu/Apollo maintenance require signed funded payers. Successful order cancellations remain free; failed, empty and replayed cancellations pay. Unauthorized Rewards updates and requested-preimage replay also pay. Successful Iroha migration stays free, with bounded voluntary sponsorship available for zero-XOR claimants.

**Bridge peers do not need XOR.** Authenticated protocol calls remain free, including accepted local failures. Invalid proofs and replays are rejected before execution; unrelated peer calls remain paid. There is no incoming fee escrow or bond. Pending-operation bounds preserve existing counts; the call-size limit and recovery requirements are detailed in the runbook.

Validation passed **1,570 native tests**, with three existing ignored tests, plus exact-Wasm fee/refund/replay and compatibility checks. The package includes the exact source patch, evidence and six unsigned governance calls. Synthetic fixtures, conservative weight estimates and operator-readiness limits are disclosed in [VALIDATION.md](VALIDATION.md).

Wasm SHA-256 (3,086,396 bytes):
`9cdc615875e0664304c50bfc09350660388e4c015543fbb4d827995af9ad9037`

`system.setCode` proposal hash (3,086,402 bytes):
`0x2ed47f947903c423083bb8cc796e121d6eb5cb7dfd7de9bcb18127be7867d16a`

At finalized block **27,907,565**, **4.8.11 remains queued**. Resolve that proposal normally, confirm funded keeper/reporter readiness, and refresh the baseline before activation. The guarded council call cannot overwrite the queue. If 4.8.11 enacts, repeat the exact-Wasm checks against its deployed runtime. Bridge peers remain exempt from XOR funding.

Verify the ZIP checksum and follow [GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md). This candidate includes the prior staking reward repair. **No governance transaction has been submitted.**
