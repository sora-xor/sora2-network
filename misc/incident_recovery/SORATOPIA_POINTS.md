# SORA Nexus Soratopia points recovery

Soratopia points shown under Polkaswap rewards are included in the recovery at
block **28,191,817**, hash
`0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0`.
The same signed account mapping and attacker policy apply to these entitlements.

The current `PointSystemV2Page.vue` reads `accountMeta` through the Polkaswap
indexer. It uses historical activity, account creation time and indexed referral
rewards, together with current wallet and liquidity positions, to calculate the
displayed score. `accountPointSystems` retains versioned activity records. Neither a new
runtime token balance nor a copy of a displayed score captures this ownership.
The existing scoring rules, historical valuations and era coefficients must be
preserved. Live position contributions remain subject to the corresponding
runtime and financial conservation checks.

The referral category comes from `referrerRewards` records fetched separately
from `accountMeta`. Its checkpoint attribution must also follow the effective
owner without counting the old and new histories twice. Migrating the on-chain
referral relationship alone does not preserve this category's score.

Reconstruct the original metadata and every existing points era through the
checkpoint before projecting ownership. Replacement owners inherit their
original owner's creation age and activity exactly once. Preserve immutable
source records, retire old query ownership and extinguish attacker entitlement
without transferring it to other participants. Reject occupied destinations,
ambiguous account aliases, incomplete replay and mismatched policies. New
activity must extend the inherited baseline without adding it a second time.
Post-checkpoint activity on the abandoned branch is excluded.

The on-chain `Soratopia.LastCheckIn` map is separate eligibility state. Generic
typed account transformation covers its key, and the final live-owner scanner
explicitly rejects retired identities in that map. A storage descriptor does
not prove that checkpoint rows exist; the off-chain points requirements apply
even if the map is empty.

The outer package requires `soratopiaAllocationPolicy`,
`soratopiaCheckpointEvidenceManifest`, `soratopiaEntitlements`,
`soratopiaRecoveryVerifier`, `soratopiaRecoveryVerifierProvenance`,
`soratopiaNodeBinary` and `soratopiaNativeVerifier`. These files alone cannot
clear `soratopiaPointsOwnershipAndConservation`. A fresh, pinned executable must
authenticate complete historical activity and valuation inputs and rederive
the exact projected ledger. A present-day database, a database alias, an export
receipt or a success flag in JSON does not establish checkpoint correctness.
The package currently leaves this gate unverified until that executable
verification is available and run; the worker migration is not a substitute.

Keep the commitments acyclic. The canonical allocation policy and ledger use
the policy commitment, genesis, checkpoint, mapping and attacker registry.
The ledger must not embed the outer manifest hash or a future fork anchor.
Live indexer admission separately binds the outer release and verified
post-fork anchor, and selects metadata by executing code identity.

Use an isolated recovery indexer database and canonical replay. Preserve the
abandoned database as evidence. This work authorizes no deployed service change
and introduces no new points claim platform.
