Council, please review **SORA 4.8.12**, runtime spec **134**, transaction version **131**.

This upgrade closes the identified fee bypasses. BABE/GRANDPA reports and Kensetsu/Apollo maintenance require signed funded payers. Successful order cancellations remain free; failed, empty and replayed cancellations pay. Unauthorized Rewards updates and requested-preimage replay also pay. Successful Iroha migration stays free, with bounded voluntary sponsorship available for zero-XOR claimants.

**Bridge peers do not need XOR.** Authenticated protocol calls remain free, including accepted local failures. Invalid proofs and replays are rejected before execution; unrelated peer calls remain paid. There is no incoming fee escrow or bond. Each proposer has a fair share of the 128 new pending multisig slots (32 with four members). Current peers can cancel abandoned proposals by quorum even when the queue is full or the proposer has left, without executing their payloads. Existing incoming failure/consumed-hash behavior is preserved.

Fresh validation passed **1,151 native tests**, with three existing ignored tests, including eleven new quota/cancellation regressions. The earlier 430 maintenance/common/inbound test results retain their historical provenance. The exact Wasm passed fee/refund/replay, fair-quota/full-queue recovery and compatibility checks. The package includes the exact source patch, evidence and six unsigned governance calls. Synthetic fixtures, conservative weight estimates and operator-readiness limits are disclosed in [VALIDATION.md](VALIDATION.md).

Wasm SHA-256 (3,089,114 bytes):
`faf9ab84f3087639c4913ada9e17ea055fffc913b936b3c366fa2791ccd5ea59`

`system.setCode` proposal hash (3,089,120 bytes):
`0xb9da4b9666d8b38b92622f6da6a97d3db783f9f1f53c9544e4f964a8c1812066`

At finalized block **27,909,040**, **4.8.11 remains queued**. Resolve that proposal normally, confirm funded keeper/reporter readiness, and refresh the baseline before activation. The guarded council call cannot overwrite the queue. If 4.8.11 enacts, repeat the exact-Wasm checks against its deployed runtime. Bridge peers remain exempt from XOR funding.

Verify the ZIP checksum and follow [GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md). This candidate includes the prior staking reward repair. **No governance transaction has been submitted.**
