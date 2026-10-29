# SORA Nexus offline recovery compiler

The Rust `incident-recovery` utility consumes a complete, independently verified
checkpoint capture. Its inventory and economic/control outputs are review artifacts;
they always report `productionReady: false`. None of these commands connects to a
node, signs, executes a historical call, changes a live database or proves finality.

## Encoding and coverage

`inventory::MetadataSchema::decode` accepts exact-roundtripping portable V14/V15
metadata. Metadata identity must also be bound by the reviewed release evidence.
`inventory::inventory` covers top storage and every captured child trie, verifies
reversible map hashers before rehashing replaced AccountIds, and requires explicit
typed preimages for hash-only keys. It does not search arbitrary bytes for addresses.
Unknown legacy or child layouts require `additionalStorage` schemas and semantic
review. Occupied replacement keys, duplicate destinations and prefix collisions fail.

Every present prefix needs an explicit disposition with a nonzero evidence hash:
`mutable_ownership`, `retain_historical`, `cancel_pending` or `manual`. The last two
require a dedicated adapter; they never mean “delete every row in this prefix.”
Generic rewriting refuses consensus, historical and authorization-bearing records.
It also rejects sorted/bounded account collections without a dedicated ordering
adapter; an exact SCALE roundtrip does not prove that binary-search or BTree ordering
is preserved after changing an account key.

`PoolXYK.AccountPools` and `VestedRewards.Rewards` may receive reviewed
`mutable_ownership` dispositions: their asset/reward collections contain no
AccountIds, so the value moves unchanged with the owner key. The exact
`Farming.PoolFarmers` roster is an unordered-owner exception: the pallet uses
linear lookup and rebuilds the roster. Other account-bearing ordered collections
still require an adapter. These dispositions need real checkpoint review evidence;
the compiler does not manufacture that approval or infer it from a sample account.

`includePinnedCheckpointLegacy: true` is an explicit, exact-metadata opt-in for the
removed storage codecs in `checkpoint_legacy_schema.json`. That file records source
revisions/hashes and 19 descriptors, including old ReferralSystem ownership pointers,
historical session roots, Democracy preimage providers/deposits, old authorship
evidence and custom staking aggregates. The codec extension does not authorize a
financial/control change. Current and historical TRANSFER permission IDs differ;
`recover_permission_preimages.mjs` recovers every component only when its exact typed
preimage rehashes to the captured key, using checkpoint accounts, assets and DEX IDs.

Typed decoding is bounded before allocating a value tree: 1 MiB per record, 65,536
nodes and depth 64. Compact sequence lengths, including zero-size-element sequences,
cannot bypass these limits. Larger records are blockers. Exact `opaqueRetentions`
may retain only the pinned checkpoint `:code` or metadata-proven byte-vector entries
in `Preimage.PreimageFor`, with exact value hash and a `retain_historical` disposition.
This preserves opaque bytes; it never approves executing their calls. The report
distinguishes `allEntriesDecoded` from `allEntriesClassified` and counts retained
opaque records. Arbitrary unknown ownership cannot use this exception.
The reviewed `EthBridge.AccountRequests` codec validates the entire fixed-width
`Vec<(u32,H256)>` without constructing a per-byte value tree, while still checking
the AccountId map key. Legacy Democracy preimages use a bounded outer-envelope
decoder: opaque call bytes remain unchanged and their provider/deposit fields are
still inventoried, including for records above 1 MiB. Those deposits and pending
authorizations retain their separate semantic review gate.

## Economic adapters

`compose-storage-work` binds economic scope to its complete native inventory.
It always watches the fixed incident AccountId32 and H160, every explicitly listed
`EconomicPlan.attackers` AccountId32, and optional additional canonical
`attackerEvmAddresses`. The plan must explicitly contain the fixed AccountId32;
an omitted position list cannot silently omit its identity from the scan. A private
in-memory certificate binds all captured top/child key-value bytes, the full
decoding registry/descriptors, the watched registry and the unmodified inventory
report. Reviewed adapter resolutions still participate in the watch. Unknown or
incompletely decoded ownership storage cannot establish absence.

The current accepted certificate requires zero direct typed identity references
and no economic operations asserting a positive interest. Any observed reference
blocks this path until a source-specific ownership/backing adapter resolves it;
neither an H160 occurrence nor an AccountId32 occurrence proves beneficial title.
The result is embedded as `attackerCoverage`; the optional
`--attacker-coverage-output attacker-coverage.json` writes a new standalone artifact
only after composition checks succeed. It explicitly does not certify beneficial
or derived ownership, complete external-network registries, historical burns or
external claims, and always retains `productionReady: false`.

In particular, `VestedRewards.Rewards` is not a generic unissued-token claim:
native claim execution transfers existing PSWAP from reason-specific reserves,
and its global `TotalRewards` affects proportional vesting. Legacy H160 rewards
also use reserve-backed transfers. Their extinction requires a separate native
aggregate/backing design, not an issuance reduction or arbitrary field zeroing.
No such destructive operation is generated when the complete typed scan observes
no direct interests of the supplied identities.

`economics::compile` consumes an `EconomicPlan` and returns exact guarded storage
edits, typed terminal-audit projections and a report. Direct native and Tokens burns
require explicit attacker ownership and unencumbered-interest evidence. Reserved or
frozen balances, or missing clearance, produce quarantine entries and no burn.
Free-balance reductions and corresponding issuance reductions are aggregated once
per asset, checked for overflow and underflow, and emitted together. They must remain
in one transactional compiler group when converted into runtime chunks. Account
provider/consumer lifecycle is a separate review gate; these adapters preserve those
fields rather than guessing which pallet owns a reference.
Write chunks retain their 128-edit limit. Exact-key audit chunks independently
allow up to 512 witnesses; both their encoded envelope and the aggregate values
read remain capped at 512 KiB. Empty-prefix audits retain the 128-prefix bound.
Packing more read-only witnesses does not omit any transformed, deleted or
preservation key. The maintenance executive charges the length-probe and value
reads, cursor/status writes, hashing and trie-path overhead separately from its
envelope charge. The final terminal audit remains mandatory.
Already-zero free balances produce no no-op account or issuance writes. Native
issuance always has a terminal invariant, even when no attacker balance exists.
`issuance_baseline` supplies unchanged-supply invariants for assets affected only
by account substitutions; a composer must preserve any existing burn/mint invariant
for the same asset and obey the terminal audit's bounded size.

Unissued reward/claim extinction requires separate unissued-liability evidence and
must not reduce token issuance. The current compiler inventories these records but
quarantines them until an exact storage-specific backing/aggregate-total adapter is
available; an evidence hash alone cannot approve zeroing an arbitrary amount field.
An escrow-backed refund cannot use an unissued-liability operation.
Scalar projections come from metadata-proven, non-compact u128 fields and pin the
hash of the complete record as well as its offset. Edits prove that all other bytes
remain unchanged.

LP proportional arithmetic is available for review, including exact integer flooring.
It does not emit pool changes. Pool fees/minimum liquidity, farming/lazy rewards,
shared ownership, debt settlement and external refund authorization need their exact
adapters/evidence. Those positions block generation of a deployable release.
Collateral references remain unchanged and must cover the reviewed minimum; overlap
with a burn/extinction or duplicate backing is rejected. Custody enforcement is not
proven merely by recording a collateral hash.

## Account-control adapters

Each `ownershipProofs` entry contains `old`, `new` and `signature`. The default
signature is a SCALE `MultiSignature` over the checkpoint-bound replacement
message. A Polkaswap migration v1 submission may instead include:

```json
"intake": {
  "version": 1,
  "nonce": "123e4567-e89b-42d3-a456-426614174000",
  "createdAt": "2026-10-27T01:02:03.004Z"
}
```

The verifier reconstructs the exact UTF-8 migration request, binding the
Polkaswap domain, SORA Nexus mainnet genesis, checkpoint and old/new pair. It
accepts the new key's raw or `<Bytes>`-wrapped signature, with or without the
scheme prefix. The nonce and timestamp must be those actually signed; the values
above are examples. Version 2 intake contains no new-key proof and remains
rejected. An old-key signature alone never establishes replacement ownership.

For every mapped owner, all 65,536 `utility::as_derivative` indices are checked
against the checkpoint's native and token account keys. Any discovered account
must appear both in the mapping and in the verified derived-control graph.
A direct signature from its proposed destination does not waive that graph.

`substitution_semantics::compile` requires replacement ownership proofs and exact
reviewed row/value hashes. It can reset `Staking.Payee` to the replacement account.
Proxy and announcement suspension candidates preserve unrelated delegates when the
owner is unchanged, but retained deposits remain custody/reclaim blockers. The old
one-row `SuspendReferral` action remains blocked; use the paired `--referral`
adapter described below to retain the forward/reverse edge and native references.
These edits must supersede overlapping generic ownership candidates, not be appended
as independent writes to the same keys.

Scalar control records (`Staking.Payee`, `Staking.Bonded`, referral pointers and
governance delegation state) cannot use generic `mutable_ownership` disposition.
They need exact adapters even when the third-party recipient/controller is not
in the replacement CSV. The production composer also adds both sides of the
actual replacement mapping to protected economic owners; callers cannot omit
that protection by leaving `protectedOwners` empty.

`compose-storage-work --staking-controls REVIEW.json` enables the paired
`Staking.Bonded`/`Staking.Ledger` control adapter. The review contains a nonzero
`evidenceHash` and `pairs`, each with canonical hexadecimal `stash`, `controller`,
`bondedHash` and `ledgerHash`. Every mapped stash must appear exactly once. The
compiler authenticates native map keys and exact values using the pinned checkpoint
metadata; a caller-supplied schema cannot waive those codecs.

For each approved pair, the replacement stash becomes its own controller. The old
Bonded and controller-keyed Ledger rows are removed and their replacements inserted
as one coupled group. Only the typed ledger stash field changes: total, active stake,
unlocking entries and legacy claimed eras remain byte-for-byte identical. The ledger
must satisfy `total = active + sum(unlocking)` and the retained 32-unlock/84-claim
bounds. Shared, orphaned, missing or ambiguous controller relationships, occupied
destinations and virtual stakers fail closed. A mapped controller of an unmapped
stash needs a separate explicit owner repair; the adapter does not inherit that grant.

The native staking hold or legacy lock must equal the ledger total and be backed by
the stash's balance and System references. Those references belong to the stash;
the deployed `set_controller_to_stash` path changes no controller reference counts.
The production composer must supply exact unchanged-byte moves for `System.Account`
and all present `Balances.Locks`, `Holds`, `Freezes` and `Reserves` records, while
preserving absence for missing records. `verify_backing_moves` accepts an identical
move from the generic inventory or another typed adapter, including the referral
adapter, and rejects missing or altered movements. Final witnesses bind the preserved
bytes at their replacement owner. The control adapter never changes balances, releases
locks, guesses consumer/provider deltas or rewrites historical exposures and liabilities.
Staking payout reset remains the separate mandatory Payee adapter.

`compose-storage-work --referral REVIEW.json` enables the deployed referral custody
adapter. Its review has `evidenceHash` and exact `records` containing `storage`,
`key` and `valueHash`. Include every affected `Referrals.Referrers` row, each
reverse `Referrals.Referrals` row that will change, and the `System.Account`
records for both endpoints of every affected edge. Missing, extra, changed or
duplicate witnesses fail. The complete forward/reverse graph is independently
checked, including duplicate and orphan entries; each endpoint must retain at
least the native consumer references required by that graph.

The adapter remaps existing edges and preserves the original reverse-vector order.
It moves each replaced `System.Account` byte-for-byte, including nonce, balances,
providers, consumers and sufficients. It does not guess whether a reference came
from genesis or a later `set_referrer` call, which have different provider
accounting. Third-party reference counts remain exact unchanged witnesses.
All groups finish under maintenance before activation permits economic calls.

For each replaced grant owner, the adapter writes a source-pinned
`Referrals.SuspendedReferrers` marker carrying the effective referrer and raw
review evidence commitment. The marker never embeds the derived recovery policy
hash. The original edge, reverse index and native references form retained
custody; `ReferrerAccountProvider` returns no payout destination while suspended.
No token balance, accrued referral-fee entitlement or reserve is released.

After recovery, only the effective owner can call
`confirm_suspended_referrer(expected_referrer)` to reactivate that exact edge.
The native call checks the retained forward/reverse/reference witnesses, cannot
redirect the edge, and removes only the suspension marker. Replays, old-owner
calls, wrong destinations and inconsistent custody fail. A different referrer
requires a separately reviewed reference-accounting operation; the old one-row
`SuspendReferral` action remains blocked. Unaffected owners retain their existing
grants when the referrer identity is legitimately replaced.

`compose-storage-work --democracy REVIEW.json` enables the pinned Democracy
delegation adapter described in [DEMOCRACY_RECOVERY.md](DEMOCRACY_RECOVERY.md).
Its exact review covers the complete voting graph, every referendum, explicit
absences for referenced cleared referenda, and mapped voting owners' original
native locks and System records. Compromised delegation grants are ended using
native immediate-target accounting and prior-lock accumulation; unaffected grants
remain directed at the effective replacement target. Direct votes, finished
referendum records and every existing lock byte are preserved. Ongoing referenda,
inconsistent aggregates and changes exceeding one atomic chunk fail closed.
The composer must verify the exact required custody moves after combining all
adapters; a voting record replacement never substitutes for moving its lock.

`compose-storage-work --governance REVIEW.json` enables the dedicated collective
adapter. The review contains a nonzero `evidenceHash` and a `records` array. Each
record has `storage` (for example `Council.Members`), canonical hexadecimal `key`,
and `valueHash` (the exact Blake2-256 value hash, or `null` for an absent singleton).
Supply the complete checkpoint inventory of `Members`, `Prime`, `Proposals`,
`ProposalCount`, `ProposalOf`, `Voting`, and `CostOf` for both `Council` and
`TechnicalCommittee`, plus `TechnicalMembership.Members` and
`TechnicalMembership.Prime`. Empty maps contribute no rows; absent singleton
records must still appear with `valueHash: null`. Omitted, extra, duplicate or
incorrect records fail. The input reader is bounded to 64 MiB; the adapter also
bounds the record count and verifies the authenticated metadata codecs.

The adapter replaces only existing members using the ownership-proven CSV and
sorts each resulting roster. It independently remaps each existing prime, which
must already belong to that roster. Unused replacement destinations remain a
global inventory requirement, including ownership references in proposals that
will be cancelled. A changed collective roster cancels every pending proposal;
otherwise proposals or votes referring to a replaced owner are cancelled.
Cancellation removes the exact `ProposalOf` and `Voting` records and corresponding
`Proposals` entries. Proposal hashes, list/map coverage and voting ownership are
checked. `ProposalCount` stays unchanged and monotonic, so old proposal indices
cannot be reused. Nonempty `CostOf` records block compilation until an explicit
custody/accounting adapter exists; this adapter creates no consideration refund.

The checkpoint's TechnicalMembership store may be absent while TechnicalCommittee
has members. Each existing roster and prime is transformed independently; the
adapter never populates an absent membership store or copies seats from the other
collective. An existing discrepancy can be retained only through its reviewed
exact before/after transformation. The compiler's private certificate binds the
replacement mapping and complete generated edits, including cancellations. It
does not authorize additional authorities or changes to the approved recovered
council. The projected Council roster must still exactly equal that supplied list.

For unaffected records, optional `governancePreservation` in the semantic plan
binds exact hashes/absence of the two technical member sets and both primes plus
an explicit review commitment. This alternative is accepted only when all four
records are unchanged and contain no mapped identity. It cannot be combined with
the transformation certificate to waive a missing repair. Neither mode changes
the recovered council's signature requirements or constitutes release approval.

Utility derivative, standard threshold multisig and custom BridgeMultisig identities
use their different canonical derivation formulas. Derived destinations must match
the substitution inventory. Every controlling key needs its checkpoint-bound proof;
the tool does not demand an impossible direct signature from a derived account.
Custom BridgeMultisig membership is checked against the checkpoint record. Membership
remapping/sorting and complete derived-account discovery remain dedicated coverage
obligations. Democracy delegation totals require the separate paired adapter above.

Exact BridgeMultisig cancellation verifies recovered opaque call preimages and every
associated pending operation/approval row. It only removes the approved bookkeeping;
it never decodes a `RuntimeCall`, dispatches it or uses nominal deposit fields to refund.
Underlying request/status/replay witnesses must remain unchanged. Shared call payloads
cannot be removed while another pending owner is omitted. New admission markers force
the exact counter-decrement adapter rather than leaving counters inconsistent.

Typed adapters emit non-deserializable `AdapterResolution` records for supported
completed operations. `inventory_with_resolutions` requires the exact captured
before hash, storage key, action, adapter identity and corresponding compiled edits.
It resolves only that record; another row under the same prefix retains its original
gate. Observed ownership references still participate in the unused-destination
check. Preserving an underlying bridge replay record is not a waiver of its ownership
or authorization requirements. Adapters that still report economic/reference-count
blockers do not provide a completed resolution.

The release compiler must resolve every report blocker, bind all input evidence and
the final chunks, prove complete account/economic coverage, and pass deterministic
rehearsals. These local adapters do not substitute for those gates.

Caller-provided `additionalStorage` codecs remain available for diagnostic
decoding and identity inventory, but cannot clear production semantic coverage.
Promote a needed codec through a reviewed, source-pinned built-in adapter, as
with the checkpoint's existing legacy layouts. A reason string and evidence hash
alone cannot turn an arbitrary byte layout into approved mutable ownership.

`compose-storage-work --staking-history REVIEW.json` enables the coupled economic
history adapter. This transforms current storage that native staking still uses
for payouts and slashing; retained block headers and Session historical proof
roots are unchanged. Authenticating an old consensus proof resolves its owner to
the replacement, so the corresponding exposure, ledger, claim flags and slash
maxima must all use that same effective identity before activation.

The review contains a nonzero `evidenceHash`, complete `records` entries with
`storage`, canonical hexadecimal `key` and `valueHash`, and `absentPrefixes`.
Records cover the exact `Staking` items listed in `staking_history::ITEMS`, plus
`StakingRewardPointsStashRemapped` at the native migration-marker key. Empty
reviewed prefixes and the explicitly forbidden pre-activation async prefixes
must appear exactly in `absentPrefixes`. These commitments are checked against
the independently verified complete checkpoint, not against an RPC omission.
The existing controller-to-stash reward normalization marker must be true;
compilation never guesses that a reward-map account is a controller. At the
checkpoint it is true, and every recorded reward-point account is already a stash.

The adapter relocates validator-keyed exposures, preferences, page claims and
slash maxima, and replaces exactly identified nominator/reporter accounts in
values. Nominator/page/queue/reporter ordering, amounts, fractions, prior spans,
claim pages and all third-party interests remain exact. `ErasRewardPoints` is the
one ordered map: replacement keys are encoded in canonical account order without
merging entries or changing any individual or total points. Legacy full/clipped
exposures and queues retain their native encodings. The adapter checks complete
page-to-overview totals/counts, claim-page ranges and slash-span relationships
before and after transformation. Unknown async queue state, malformed values,
occupied destinations, missing records and ambiguous financial relationships
fail compilation.

The caller must run the compiler's private `verify_groups` certificate against
all assembled groups. It rejects omitted coupled edits, claim resets, new async
queue records, altered unchanged economic records and a different replacement
mapping. Final witnesses cover every existing and relocated reviewed record.
The separate Ledger/Bonded and payout-control adapters remain mandatory. Current
validator/nominator role grants and election-list membership require their own
review; this adapter does not grant a role, change nomination targets, or approve
an operator key.

BagsList ownership moves additionally pass the mandatory complete graph verifier.
It checks the native node key/id, count, reciprocal links, bag head/tail and unique
coverage before and after all groups. The projected graph must be the exact
identity-renamed graph, retaining saved scores, bag placement and within-bag
order. It neither sorts accounts within a bag nor recomputes scores. Only the typed
role compiler described below can authorize native-chill node removal. Unknown
maintenance cursors or queue state fail closed. Final graph witnesses include
unchanged neighbours and absent old node keys.

Empty namespaces use a separate `PrefixAudit` action in the same committed audit
cursor as exact-key witnesses. It checks both the exact prefix key and its next
storage key, in the specified top or child trie. This prevents an omitted new
record from passing a list of individual absence checks. The composer canonicalizes
and bounds prefix audits, rejects overlapping namespaces, and executes every
audit before the terminal conservation check. The mixed-work assembler also
rejects contradictions between a present-key witness and an empty namespace.

`--scheduler` supplies the complete checkpoint review for the six retained stale
Root schedules. The adapter clears their agenda rows and `IncompleteSince`
without executing the historical calls or invoking native cancellation that
could move deposits. Preimage bytes, legacy statuses and provider backing are
preserved. A mapped or attacker provider requires a separate custody adapter
and fails compilation here. Entire Scheduler and native Multisig pending
namespaces must be empty at the final audit. Complete static work is checked
against the private custody certificate; dynamic bridge work still requires
exact-artifact execution.

For read-only validation of the complete checkpoint relationships, run
`verify-checkpoint-controls --capture PATH --staking-history REVIEW --democracy
REVIEW --scheduler REVIEW`. This uses an empty mapping deliberately and cannot
approve victim identities or a release. It validates the staking page, claim
and slash relationships, the voting graph, retained referenda and schedule
custody against metadata executed from checkpoint Wasm.

`compose-storage-work --staking-roles REVIEW.json --operators-scale operators.scale`
handles current nomination and validator roles together with their BagsList nodes.
The review supplies `evidenceHash`, `operatorsHash` (Blake2-256 of the complete
canonical operator SCALE file), the complete eight-prefix `records` inventory,
and optional `retainedNominations` entries containing `old` and a SCALE-encoded
`MultiSignature`. Record fields are `storage`, `key`, and `valueHash`.
The eight prefixes are both staking role maps and their counters, BagsList's two
maps and node counter, and BagsList's storage version. The operator decoder uses
the exact runtime type with a 4 MiB allocation limit, checks domain-specific key
uniqueness and fresh/retired GRANDPA separation, and requires the
`runtime-work-types` feature.

A mapped nominator defaults to native chill: remove its nomination role and voter
node, repair exact neighbouring/head/tail links and counters, and leave all funds,
ledgers, payees and historical exposures/liabilities intact. The unused native
`suppressed` field is never treated as an enforcement mechanism. To retain a
mapped nominator's current targets, the replacement key signs
`nomination_confirmation_message`, binding both owners, checkpoint/genesis,
original nomination-value hash, the exact effective target vector in its original
order, and operator-inventory hash. This only reconfirms the existing selection;
it cannot introduce targets. A chilled replacement can nominate afresh normally.

Unmapped role owners retain their exact roles. A mapped validator retains its
role only when its original owner is covered by the supplied operator inventory;
an uncovered mapped role defaults to native chill. A retained nomination whose
target is replaced requires that target's operator record. If it is missing,
compilation fails pending the public inventory expansion; it does not suspend an
innocent third-party nominator or invent an approval. Active session/key coverage
remains an independent mandatory consensus gate.

The raw operator file is not evidence of operator approval. Release admission must
match the role review's hash to the exact bootstrap-pinned operators bytes and
verify the approved inventory evidence. The role report deliberately sets
`operatorApprovalVerified` and `productionReady` to false.

The private role certificate binds the complete before/after role and graph
projection. Only it can authorize the BagsList verifier's mapped-role removals.
Unchanged voter order/scores, exact role membership, native counted-map counters,
and the full surviving graph are checked. Unique guarded edits may span bounded
maintenance chunks; the complete projection and final witnesses must pass before
activation permits election hooks. Callers must validate `verify_groups` and use
`graph_certificate()` with the graph verifier; a partial deletion or unreviewed
node removal cannot pass either certificate.

The ElectionsPhragmen compiler accepts `--elections REVIEW.json`. The complete
review lists `Members`, `RunnersUp`, `Candidates`, `ElectionRounds` and every
`Voting` record as `{storage,key,valueHash}`; singleton absence is explicit with
`valueHash:null`, and an empty Voting map uses its prefix and null. The compiler
pins the checkpoint metadata and verifies native account key hashes, roster
ordering/disjointness, stored vote bounds, recorded deposits, and the exact
`phrelect` locks. The `evidenceHash` identifies the reviewed input; it does not
replace new-key signatures or the account's pre-compromise control audit.

Mapped voter ownership moves to the replacement and its target vector becomes
empty. Stake, deposit, account references and lock bytes remain unchanged, and
the voter row remains present. Native election input construction drops empty
approval edges, including prime votes. A replacement can use the existing native
vote/remove calls to authorize a new grant or release its own deposit and lock;
recovery itself creates no refund. Unmapped voters retain their target order and
multiplicity, with only mapped candidate identities replaced. Existing members
and candidates retain their exact recorded financial interests and are ordered
by their effective account IDs; runners-up retain merit order, including the
original order of ties. The election round counter is unchanged. The separate
collective compiler handles Council state independently; neither compiler
creates a roster from the other.

`Compilation::verify_groups` is mandatory after all work groups are composed. Its
private certificate checks the entire election projection and requires exact
unchanged-byte moves of mapped participants' System.Account, Locks, Holds,
Freezes and Reserves from generic inventory or another typed adapter. Final
witnesses bind the liabilities and absence checks. Missing/changed balance moves,
occupied destinations, omitted vote suspensions, altered rounds, and extra
writes to the reviewed election namespace fail. The native election regression
is a separate execution gate; structural compilation is not proof of governance
approval or runtime execution.

For a public empty-map diagnostic review from the authenticated local capture:

```sh
node misc/incident_recovery/prepare_checkpoint_elections_review.mjs \
  artifacts/recovery-28191817/checkpoint \
  artifacts/recovery-28191817/fixtures/elections-review-v1
```

This helper refuses overwriting a directory and marks its output fixture-only.
The actual checkpoint has 8 election members, no runners-up or candidates, and
260 voter records. These observations are not replacement or release approval.
Legacy staking role review likewise preserves existing nomination multiplicity
and empty targets: deployed `nominate` did not deduplicate and native `kick` can
empty a target vector. The async snapshot skips empty vectors. Recovery never
reinterprets these retained records using new dispatch-time sorting rules.

`prepare_victim_fixture.mjs NEW_OUTPUT_DIRECTORY` prepares an explicitly public
one-account substitution diagnostic from the independently verified checkpoint.
It derives a published Ed25519 test destination, signs the real ownership domain,
and writes a CSV, ownership proof, complete review inputs, conspicuous
`NEVER-DEPLOY.txt`, and a content-bound fixture provenance manifest. The old key
is never used. `COMPOSE-ARGS.json` records the intended production compiler
arguments, but the compiler must still verify the entire checkpoint, execute its
metadata, check unused destinations, and reject all unresolved semantic records.
The complete BridgeProxy pair review and terminal-outgoing tombstone input are
included; their typed compiler and native execution gates remain mandatory.

The selected diagnostic owner has three nonzero token balances, a referral grant,
a nominator/list entry, a zero-principal staking ledger with 84 retained claimed
eras, offence-history references and five completed bridge records. This tests
real checkpoint ownership and control coupling; it does not substitute for the
native positive-stake payout/slashing regressions. Fixture classifications retain
unaffected checkpoint rows and do not waive any mapped mutable reference. The
fixture omits attacker settlement and actual victim, council, operator and
historical-control approvals. Its published test key makes it unsuitable for any
network release regardless of whether structural compilation succeeds.

Large native composition reports use the dedicated public fixture transport:

```sh
python3 misc/incident_recovery/fixture_work_transport.py \
  --source artifacts/recovery-28191817/fixtures/EXAMPLE/compiled-work.json \
  --destination artifacts/recovery-28191817/fixtures/EXAMPLE/work-transport-v1
```

Pass its `public-work-index.json` to `prepare_maintenance_fixture.py --work`.
The new directory retains the complete native JSON report verbatim and stores
every apply/audit envelope, terminal envelope and policy as an individually
hashed SCALE file. Parsing streams envelope arrays one item at a time, with a
512 MiB report bound, 64 MiB ordinary-field bound, 576 KiB envelope bound and
16,384 entries per envelope array. The policy keeps its existing 2 MiB bound.
Loading checks the entire retained report hash, strict JSON syntax and duplicate
keys, exact metadata and policy identity, and every indexed shard against its
original report value. Iteration checks each shard again before use. The general
review-JSON reader remains limited to 64 MiB. The manifest provenance records both
the small index hash and original native report hash; transport does not assert
victim authorization, production readiness or exact Wasm execution.


Derived ownership verification follows an acyclic, bounded controller graph.
A utility derivative or multisig may itself control another derived account;
it supplies its exact old/new derivation and ultimately relies on the existing
checkpoint-bound controlling-key signatures. Derived accounts never require an
invented direct signature. Every changed graph node must match the same
substitution mapping. An unchanged derived controller is explicit proof-only
context (`old == new`, absent from the CSV); it is reported separately and never
adds a retired identity or an account replacement. The total graph, including
proof-only context, is bounded to 4,096 nodes. All controlling keys (including
unchanged signatories) still require proofs, and every registered BridgeMultisig member set is checked against its
checkpoint record. The verifier accepts native threshold-one multisig derivation
with two or more sorted, unique signatories. It rejects cycles, omitted nodes,
changed mappings, invalid leaf signatures and paths deeper than 32 derivations,
including paths through already-verified shared nodes. Input order does not
change acceptance. This control proof does not replace complete state coverage,
pre-checkpoint control-history review or custody/accounting adapters.
