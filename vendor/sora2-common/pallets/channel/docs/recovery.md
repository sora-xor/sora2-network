# SORA Nexus recovery bridge controls

The persistent raw storage key `:sora-nexus:recovery:bridge-halt:v1` is a presence
latch. Malformed or empty values still halt execution. No bridge extrinsic clears
it. It survives runtime 138 activation. Outbound submission, batching, HASHI
workers/approvals/retries, data-signer approvals, asset locking and refund paths
remain stopped. Abandoned outbound status reports cannot change a quarantined
liability or trigger an automatic refund. Previously issued external signatures
remain redeemable unless the counterpart independently invalidates them.

Normal inbound admission also stops. The internal recovery executive may set
`:sora-nexus:recovery:bridge-inbound-context:v1` to the exact SCALE u32 current
block number inside one transactional operation. Trailing bytes, a different
height or absent context are rejected. No public call creates this context.

`runtime/src/incident_recovery_incoming.rs` adds a second, narrower authorization:
only a manifest-committed typed operation may reach its message dispatch wrappers.
Its domain-separated hash occupies the active work policy's next chunk slot.
The exact source network, original commitment/proof, message identity, payload,
timepoint, application source and reviewed attacker registries are bound. Each
source network needs either explicit attacker accounts or an explicit attestation
of no known attacker accounts. Empty missing evidence is rejected. The published
SORA Nexus attacker is always required in the receiving network's registry; the
published Ethereum attacker is required for Ethereum mainnet operations.

The native channel verifies signatures and nonce progression on the unchanged
payload. The application callback then resolves the beneficiary using the signed
replacement policy. It checks native application source, registered token/kind,
precision conversion and positive amount even when excluding an attacker.
Excluded transfers consume their native nonce without minting or unlocking. For
SubstrateApp transfers, a `Done` acknowledgement is journaled: `Failed` would
invite an external refund and defeat extinguishment. The exact acknowledgement
is reconstructible from network, message ID and stored `MessageStatus`.

`SubstrateApp::DeferredTransferResults` journals both credited transfers and
excluded transfers without touching an outbound queue or nonce. It cannot be
overwritten or replayed. There is no automatic flush. A later counterpart
reconciliation release must authorize acknowledgement delivery and outbound
reopening. Ordinary mint failure remains distinguishable from exclusion.

One catch-up operation is transactional across native nonces, credits, deferred
acknowledgements, immutable recovery receipt records and the recovery cursor. A
failed application rolls all of these back, then freezes the committed work for
forward repair. An uncommitted operation cannot mutate or freeze recovery.
Original lineage and the quorum-installed active work policy are tracked separately.
The bootstrap must charge the returned mandatory weight and allow at most one
operation per block. Historical dispatch weights remain exactly unchanged when
the latch is absent; executing metadata still needs the release's code witness.

The module supports native Substrate/Parachain transfer batches, generic TON
transfers, and HASHI incoming transfer imports or completion of already prepared
transfers. EVM application transfers require the existing `wip` feature and are
not enabled by this release. Frozen-head and delivery evidence hashes bind
independently reviewed release evidence; they do not prove external finality on
chain by themselves.

HASHI operation templates bind the exact original request, network registries,
peer accounts, native BridgeMultisig calls, and native SignedExtra bytes. Only the
signature witnesses are omitted from the operation commitment. Produce a recovery
Tick child first, then collect signatures over the native SignedPayload using a
mortal era whose birth block is strictly after the checkpoint. That header hash
binds signatures to this recovery branch while avoiding a circular manifest hash.
The era and each expected nonce must be chosen in the committed template; expired
or mismatched templates require an authorized forward repair. Signature witnesses
must be retained alongside the immutable template in the execution evidence.

The adapter uses native signature checking, native nonce validation/preparation,
and native BridgeMultisig protocol admission and dispatch. It does not synthesize
a bridge origin or run economic fee hooks. Every uncommitted signature witness is
checked before any state write, so a corrupt witness cannot freeze committed work.
Only incoming import/finalize calls are admitted. Unknown external sender identities
are rejected: a zero/default sender is not evidence that a receipt is innocent.
The exact peer-approved request remains in native request and replay storage.
Beneficiary resolution occurs only in the authenticated native transfer handler.
Excluded unissued sidechain claims mint nothing. Owned, reserved entitlements are
burned in place, with issuance reduction; completion of already prepared sidechain
claims reverses their native preparation accounting. No outgoing cancellation,
refund, registry change or arbitrary call is admitted by this envelope.

Release gates remain the real reconciliation inputs and post-bootstrap peer
proofs, exact legacy-bootstrap compilation and state-root replay, full/warp
synchronization, bounded-weight measurement and production exact-artifact
rehearsals. Prepared historical HASHI requests without an authenticated external
sender require an explicit reviewed reconciliation before admission; this adapter
fails closed rather than inferring a sender from unrelated identities. No service
changes, counterpart changes, publication or restart are performed here.
