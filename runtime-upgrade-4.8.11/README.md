# SORA 4.8.11 — staking reward compatibility

This candidate restores available staking rewards in the unmodified public
Polkadot.js Apps. Runtime spec version is **133**; transaction version stays
**131** because call and signed-extension encodings are unchanged.

SORA still stakes XOR and pays rewards in VAL. `staking.erasValidatorReward`
now publishes each completed era’s recorded VAL budget. Native currency payout,
XOR stake compounding, native inflation capping, and native reward remainder
issuance are disabled for SORA’s replacement reward handler. Standard
`staking.Rewarded` events describe actual successful VAL payments, alongside
`xorFee.ValStakingRewardPaid`, and ordinary direct, paged, utility, and multisig
payout calls retain atomic claim bookkeeping.

The one-time migration copies existing VAL budgets into existing completed-era
reward entries inside the 84-era claim window. It preserves claims, exposure,
validator preferences, and reward points; it does not create active/future era
payouts, reopen paid pages, or reconstruct expired reward entitlement. Future
eras publish their budgets through the normal staking era-close path.

Public Apps may continue labeling the standard reward number with its native
XOR symbol. The actual payment asset is VAL. Generic exposure, multi-page and
rounding estimates can differ from actual payouts; the runtime remains the
authority for payable amounts and claim status.

## Activation

Restarting nodes alone cannot change finalized runtime state. Activate the
validated Wasm through the existing SORA runtime-governance route. After
enactment, verify spec 133, migration marker
`runtime:migrations:val_staking_rewards_published`, completed standard budgets
equal the recorded VAL budgets, unchanged claim markers, and actual ordinary
payouts transferring VAL without adding XOR rewards or stake.

The exact candidate Wasm and ordinary payout path passed the pinned mainnet-state
rehearsal. See [VALIDATION.md](VALIDATION.md) for evidence and limits, and
[GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md) for the prepared activation calls.
No governance transaction has been signed or submitted by preparing this candidate.
