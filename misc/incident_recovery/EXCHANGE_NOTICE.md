# SORA Nexus recovery — exchange notice (unpublished draft)

A coordinated recovery is being prepared after compromised account keys. This
draft does not announce a restart time or establish that a release is ready.
Use only the final signed release identity and coordinated operator notice when
resuming integration.

The recovery retains the complete post-state of **block 28,191,817**:
`0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0`.
**All on-chain activity after that block is reverted**, including transfers,
deposits, withdrawals, swaps and other trading activity. Reconcile your internal
customer ledger against the retained branch; confirmations on the abandoned
branch do not constitute recovery-chain settlement.

Approved compromised accounts will be replaced by public, evidence-backed
accounts throughout mutable ownership/control. The old identities will be
retired. Publish or consume the final release-bound mapping only after it is
available; do not infer replacements from transaction history or unsolicited
messages. Preserve original identifiers as audit evidence while associating
recovered positions and entitlements with their effective owners.

The intended node is **4.8.13**, with automatic final activation of **runtime
138 / transaction version 134** after maintenance. Network/database identity
changes while retained history and checkpoint hash remain intact. Metadata must
be selected by executing-code identity during the spec-134 recovery interval.
Use the verified post-fork anchor before crediting new activity.

Bridge outbound execution remains halted after runtime activation. Existing
external withdrawal authorizations may remain redeemable, and uncertain
checkpoint liabilities remain quarantined. The rollback itself is not evidence
that a bridge withdrawal failed externally. Incoming catch-up uses only approved,
authenticated commitments and consumes each message exactly once. Do not create
parallel manual credits for the same incoming evidence.

TS/SS allocations preserve original successful XOR Burn-event ordering, rates,
rounding and cap consumption through the checkpoint. Replacement-owner queries
will expose recovered entitlements. New TS/SS payout contracts and claim websites
are outside this restart. Linked SORA Nexus XOR claims involving replaced accounts
require replacement-key authorization of the payout recipient.

Keep deposits and withdrawals coordinated with the recovery operators until the
final release, reconciliation and restart criteria are met. Retain abandoned-
branch records for customer reconciliation and incident evidence.
