# SORA Nexus bridge ownership, custody and signing cancellation

The `bridge_history` compiler pairs the complete checkpoint `BridgeProxy.Senders`
and `BridgeProxy.Transactions` inventories. It requires canonical native and
checkpoint-metadata decoding, both map hashers, unique network/message identities,
matching local owners, and exact reviewed value hashes. The bounded review holds
at most 200,000 rows; native value bytes are limited to 64 MiB in aggregate and
32 KiB per record.

For a replaced owner, a terminal `Done` or `Refunded` request moves its transaction
key and sender index together. Only the local financial owner field changes:
`source` for outbound transfers and `dest` for inbound transfers. Amount, asset,
status, timepoints, message identity and every external address remain exact. An
external AccountId32 that happens to equal a replaced local account is still
external evidence. An affected nonterminal incoming request still requires authenticated
incoming reconciliation. For nonterminal outgoing requests, the explicit
`quarantineAllNonterminalOutgoing: true` policy admits a separate custody
disposition through the same complete-prefix certificate.

Four actual checkpoint rows violate the native local-owner invariant. An explicit
`preservedAnomalies` review must pin both sender and transaction keys, both value
hashes, and source interpretation evidence. The compiler accepts only otherwise
unrelated anomalies: a byte match for any old/replacement identity or either
attacker identifier in either key/value fails closed. Both original rows remain
byte-for-byte unchanged. Their direction, amount and financial owner are not
inferred from the misleading native decoding. They remain listed under
`unresolvedQuarantinedAnomalies`, and `completeFinancialInterpretation` is false.

These anomalies receive a distinct opaque `snxanom1` marker at the same native
message-guard key. Its codec is `(tag, checkpointHash, senderValueHash,
transactionValueHash, sourceEvidenceHash, reviewEvidenceHash)`; it contains no
owner, amount or refund right, and must never be decoded as `snxhist1`. The
separate native status and incoming credit guard regressions are required before
claiming execution containment. Financial reconciliation remains outstanding
even once those guards pass. The helper `prepare_bridge_history_anomalies.mjs`
binds this exact preservation review to the full baseline inventory and verified
checkpoint pages, with source hashes and an explicit historical-codec uncertainty.

Each affected terminal outbound request receives an inseparable historical
message tombstone. The explicit review action `cancelAllTerminalOutgoing: true`
also suppresses refunds for every unaffected terminal outbound request. The
report separately counts nonterminal outgoing requests; this action does not
certify their reconciliation, a batch preimage or any external authorization.

The marker's `snxhist1` codec commits the original native record hash, original and
effective owners, raw review evidence, and `refund_authorized: false`. It lives
under `:sora-nexus:historical-outgoing-message:v1:` so generic recovery work never
gains access to the executive's protected control namespace. Its evidence hash
must bind source inputs, not an output embedding the resulting policy commitment.
Native message guards consult this namespace as well as native outgoing-adapter
tombstones. Even malformed marker bytes block execution. Neither marker creates
a new claim, releases collateral, refunds a completed debit or invalidates an
already published external signature.

The complete composed write set must pass `Compilation::verify_groups`; omitting
an owner move, a sender update or a tombstone, changing an unrelated record, or
substituting another mapping fails the final projection. Exact value witnesses
cover the paired inventory and resulting markers. The runtime regression invokes
the real native status update, refund and outgoing-record constructors after
clearing the independent global halt, and requires unchanged records, balances
and issuance.

`checkpoint_bridge_history.mjs` creates an offline diagnostic review from every
checksummed capture page and the executed metadata witness. It checks capture
cursor coverage and native map key/value encodings; Rust compilation independently
checks the pair graph. Output is explicitly a public diagnostic input, never
victim authorization, source-chain finality evidence or release approval.


Nonterminal outgoing requests retain their exact native status, amount, asset,
external recipient and timepoints. The new `snxpend1` marker binds checkpoint,
original transaction and sender hashes, original/effective local owners and raw
review evidence. The ownership move, sender update and marker are one atomic
group. It authorizes no refund. A known attacker owner's entitlement is marked
extinguished without crediting an account or releasing custody; other owners'
uncertain liabilities remain quarantined. The native transaction remains evidence,
not a new claim. Missing external redemption/signature evidence is not inferred.

Queued channel messages are checked using the native message-ID construction
(source network, destination network, next batch nonce, exact position) before
taking the queue or advancing its nonce. A quarantined position also prevents
appending new work behind it. HASHI compares the native outgoing request hash to
the same BridgeProxy message-ID domain before signing, retrying or refunding.
These stops survive removal of the independent global halt.

The `bridge_signing` compiler additionally requires the complete checkpoint
`BridgeDataSigner.Approvals`, `PendingApprovals` and both outbound channel
`LatestCommitment` inventories. It validates their exact native codecs, computes
hashes only from known latest commitment preimages, and quarantines the union of
native signing hashes. Existing signature bytes, flags and commitment bytes stay
unchanged. Empty prefixes receive committed prefix audits; every retained row
and every new marker receives an exact audit witness. A private whole-prefix
certificate rejects omitted markers or any alteration of native evidence.

The separate `snxsign1` namespace stops approval, re-registration and clearing of
these known hashes through the existing native `commitment_quarantined` guard.
It never establishes the membership of an unknown historical batch and never
invalidates signatures already published on an external network. Exact frozen
counterpart evidence is still required for any later refund or reopening.

Native HASHI has older outgoing requests that never received a BridgeProxy row.
The `bridge_hashi` compiler therefore traverses all seven native namespaces:
`Requests`, `RequestStatuses`, `RequestApprovals`, `RequestApprovers`,
`RequestSubmissionHeight`, `RequestsQueue` and `AccountRequests`. A compact review
pins the complete sorted inventory of each namespace. Starting with
`blake2_256("sora-nexus-hashi-prefix-v1:" || storage_name)`, each row replaces the
commitment with `blake2_256(SCALE(previous_hash, raw_key: Vec<u8>, value_hash))`.
The native compiler independently checks the exact count and digest. Bounds are
1,000,000 original rows and 512 MiB aggregate original values.

Every decoded native outgoing request receives a separate `snxhash1` permanent
stop at `:sora-nexus:historical-hashi-request:v1:`. The identity is its captured
map key, checked against the native wrapper's stored hash and outgoing network.
It is never reconstructed by hashing a potentially migrated amount. The marker
binds checkpoint and evidence, original request/status hashes, original/effective
owner, the captured transfer fields where applicable, attacker extinguishment
and `refund_authorized: false`. Administrative requests confer no financial
entitlement. Captured amounts remain evidence; this action does not certify that
a liability is refundable or that collateral may be released.

Exact terminal witnesses cover each outgoing request's payload, status,
approvals, approvers and submission height, including absent records. Original
signatures and native requests stay byte-for-byte unchanged. The private
certificate also protects every original namespace against static work-set edits.
It does not impose an unchanged whole-queue terminal witness: authenticated
incoming processing must be able to consume its own pending queue IDs. Native
HASHI guards prevent a cancelled outgoing ID from being processed or refunded.

A mapped `AccountRequests` index can move, without changing its contents, only
when every referenced native wrapper has that exact original author, network and
stored request identity. Mixed outgoing/incoming indexes preserve their original
order. Unknown or duplicate IDs, inconsistent ownership, occupied destinations or
indexes exceeding 128 KiB fail closed and require explicit reconciliation.
Cancelled outgoing payloads and signer evidence remain historical evidence.
Their sources receive read-only audit witnesses, never no-op storage edits; the
private certificate binds them to the exact permanent stops and prevents later
static edits. This does not remint failed requests or refund post-checkpoint
debits already undone by rollback.

Incoming transfers and their transaction-load records use a distinct `snxhown1`
ownership certificate under `:sora-nexus:hashi-incoming-ownership:v1:`. It binds
the original request/status hashes, checkpoint, evidence, original/effective
author and, for transfers, original/effective beneficiary independently. A load
author does not acquire the beneficiary's entitlement. This certificate creates
no cancellation, execution permission, mint, refund or assertion of receipt
completion. Exact signed request bytes and stored replay identities remain
unchanged. Mapped incoming administrative requests and nonterminal legacy
`TransferXOR` loads require separate reconciliation and fail compilation.

Authenticated catch-up can reuse an exactly identical checkpoint `Pending`
transaction-load record only inside the guarded recovery inbound context. The
native queue must contain it exactly once and the external replay identity must
be unconsumed. Fresh native peer quorum then runs the existing registration and
finalization routines; it never re-registers the old load or changes its author,
kind, timepoint or submission height. A prepared incoming transfer instead uses
its exact native finalization path. Credits resolve the committed replacement
beneficiary after authentication, and query entries resolve their independent
author. Failure rolls back balances, nonces, votes, indexes and replay state.
Completion requires the native `Done` state, consumed external replay mapping,
removed queue entry and recovery receipt. A second delivery cannot credit again.

The verified checkpoint inventory has 85,147 incoming transfer records, all
`Done`; its transfer-load records contain 85,147 `Done` and eight `Failed`, plus
one failed `TransferXOR` load. It has no `Pending` transfer/load records. This is
an observation of retained local state, not proof that counterpart deposits or
post-checkpoint receipts are complete. The release must still reconcile and
authenticate every outstanding external transfer. Legacy Ethereum XOR
decommission checks remain enforced; a failed special load is never silently
reinterpreted as an ordinary transfer.
