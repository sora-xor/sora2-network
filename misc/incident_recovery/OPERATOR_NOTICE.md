# SORA Nexus recovery — operator notice (unpublished draft)

This notice describes the proposed recovery release. No release publication,
operator upgrade, signing stop, service containment or restart is asserted here.
Only a complete, verified outer release manifest and the coordinated restart
notice may supply production commands, authority inventories and artifact hashes.
The source templates deliberately cannot start production recovery unbound.

The retained checkpoint is the complete post-state of block **28,191,817**,
hash `0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0`.
All later activity on the abandoned branch is reverted. Historical blocks and
finality evidence through the checkpoint must remain available.

The intended release is **node 4.8.13**, an immutable legacy **spec-134**
substitute, a pinned **spec-135** hold runtime, and final **spec-138 / transaction
134**. The runtime-137 council package is superseded. Runtime 138 must be
installed by the recovery executive after its committed work and invariant
checks; it is not an ordinary council upgrade on the abandoned chain.

## Before restart

1. Stop all signing on the abandoned branch. Record the final signed BABE slot
   and the required public signing-stop evidence. The first recovery slot must
   exceed every reported signed slot; the release cannot guess this number.
2. Preserve an immutable copy of the abandoned database and its evidence. Prepare
   a **separate** database ending at block **28,191,816**. Every participating
   restored node must import the checkpoint under the recovery release. Do not
   edit or delete historical consensus evidence in place.
3. Use the release's new chain-spec ID, database path and protocol/fork identity.
   Runtime overrides are forbidden. The spec-134 substitute must remain byte
   identical for the lifetime of this recovery chain, including future syncs.
4. Supply freshly generated GRANDPA **public** keys using the approved operator
   inventory. Private keys stay with their operators. Remove the manifest-listed
   retired GRANDPA signing keys from the recovery keystore, including retired
   active, queued and pending keys. Confirm independently retained BABE and
   liveness keys against the approved inventory.
5. Verify that the release pins the client set ID immediately before checkpoint
   import and applies exactly **S+1** to both ordinary import and warp sync.
   An arbitrary high-bit authority-set reset is not supported.
6. Verify all manifest file hashes, policy/data commitments and exact-artifact
   rehearsal results. Check the account mapping, derived accounts, council,
   operator evidence, finalized external heads and bridge reconciliation evidence
   against their approved public records. A candidate package marked incomplete
   does not authorize restart.

## During recovery

The maintenance executive accepts only timestamp/consensus handling, committed
recovery work and the quorum-authorized escape path. Economic sessions/eras,
ordinary transactions and economic hooks remain frozen. Node offchain workers
remain suppressed while historical state is imported and while maintenance runs.

Recovery envelopes are selected by exact parent hash. A missing envelope produces
a maintenance tick; it does not approve skipped work. Restart resumes the stored
cursor. A committed invariant failure requires investigation or the pinned
recovered council's **strict majority** escape to hold 135. Signatures must bind
the complete action, checkpoint, policy, phase, cursor, code identities and
persistent operation counter. Versions 136/137 are reserved for reviewed forward
repairs. Never replace the immutable 134 substitute to repair finalized history.

After the terminal audit and all pinned final-code chunks are present, activation
of runtime 138 is automatic. The deferred session transition receives the
existing one-session liveness grace. Kensetsu's fee clocks exclude maintenance
downtime once, while pre-checkpoint debt remains.

## After activation

The bridge's on-chain outbound and worker halt remains active. Reopening is a
separate coordinated action after external nonces and withdrawal authorizations
are reconciled. Previously issued external authorizations may still be redeemable;
the local fork cannot invalidate them. Do not advertise cancellation as proof of
external non-redemption, or manually refund quarantined liabilities.

Indexers and wallets must use the recovery fork anchor and executing-code
identity. Spec version alone cannot distinguish the deployed 134 runtime from
the immutable 134 substitute. Confirm TS/SS entitlement queries use the original
Burn-event identity and the effective replacement owner. Linked SORA Nexus XOR
claims for replaced accounts remain held until replacement-key recipient
authorization is verified.

Soratopia points also follow the replacement owner. Preserve checkpoint activity,
account creation age and each existing points era in the recovery indexer. Do not
copy present-day totals from the abandoned branch or add the old and replacement
totals together. Keep original records as evidence; retired identities must not
retain a second spendable or queryable entitlement. Check the separate Soratopia
replay and conservation gate before admitting the recovered rewards service.
