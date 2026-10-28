# SORA Nexus outgoing quarantine

`runtime/src/incident_recovery_outgoing.rs` implements an internal maintenance
operation, transported by `RecoveryAction::Outgoing`. Its domain-separated SCALE
commitment occupies one entry in the active recovery work policy. One operation
runs transactionally with the persisted recovery cursor. It does not transfer,
unreserve, mint, burn, or create an entitlement to a refund.

The HASHI target authenticates the complete native outgoing request and status by
hash, recomputes its native request ID, and verifies network, author and exact
transfer asset/amount (or the absence of transfer fields for a control request).
The original request, status, approvals, queue and escrow remain intact.

The channel target authenticates the entire native uncommitted queue or the
native latest commitment. An older commitment requires its exact canonical SCALE
preimage, its native protocol hash in retained DataSigner pending/signature
storage, and a hash of that complete signing state. Missing native evidence is
not accepted as an unsigned or safely refundable withdrawal. Every message must
match its native position, payload fields and message ID. Financial messages
must additionally match the native BridgeProxy sender and complete outbound
transaction, including owner, asset, amount and status. Unindexed protocol
messages create no financial claim. Partial batches, reordered messages and
nonce mismatches are rejected.

The records use source-pinned keys from `bridge_types::recovery`. Request and
message tombstones stop native signing, retry, execution and automatic refund
paths after the independent global halt is removed. A channel batch tombstone
blocks its exact nonce without removing or renumbering messages. A commitment
tombstone blocks DataSigner approval, reopening of the approval window and
signature clearing. HASHI incoming cancellation and completion reports resolve
the referenced outgoing identity before any status mutation. The legacy XOR
aggregate decommission helper exits without changes on recovery lineage so it
cannot destroy quarantined backing.

Victim/third-party contingent rights identify the policy-resolved replacement
owner. They are not a new recognized debt: every record explicitly sets
`refund_authorized = false`. Terminal `Done`/`Refunded` transactions create no
financial claim. Exact attacker-owned contingent claim rights are marked
extinguished, without burning escrow or collateral. Independent typed backing
accounting is still required before any later refund or burn. An external
recipient address does not establish ownership of the local liability.

Uncommitted or malformed action data cannot advance or freeze the cursor. A
committed native preimage mismatch freezes recovery with no partial bridge
mutation; the pinned council escape remains available. Duplicate tombstones are
rejected. Recovery storage and native execution tests cover cursor behavior,
full-batch membership, archived signing evidence, preservation of native records,
and rejection of refunds/signatures after clearing the global halt.

Release composition must enumerate every checkpoint outgoing request, queue and
retained signing commitment, and bind external reconciliation evidence. This
module is not an inventory-completeness certificate. Approvals already public on
the abandoned branch remain usable on an external counterpart; local tombstones
cannot revoke those signatures. Outbound reopening requires a separate review
of counterpart nonces, authorizations and all quarantined liabilities.
