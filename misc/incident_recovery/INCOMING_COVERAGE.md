# SORA Nexus incoming obligation coverage

The release verifier now checks the finite incoming work against the complete
native HASHI transfer/load inventory and a separately supplied external delivery
inventory. An omitted local obligation or declared external delivery is an error.
This is the `incomingPendingCoverage` gate, separate from `bridgeReconciliation`.
It does not authenticate an external finalized head, prove a source archive is
complete, validate signatures, mint funds or establish runtime execution.

Run the fixed native command on public release inputs:

```sh
incident-recovery verify-incoming-coverage \
  --capture /absolute/checkpoint \
  --policy /absolute/release/policy.scale \
  --work /absolute/release/work/index.json \
  --inventory /absolute/release/incoming-deliveries.json \
  --reconciliation /absolute/release/incoming-reconciliation.json \
  --external-heads /absolute/release/external-finalized-heads.json
```

The command reconstructs the checkpoint, executes its own Wasm to authenticate
metadata, and decodes every native HASHI request. Request map keys and stored
wrapper hashes are authoritative; signed financial payloads are never rehashed
to invent replacement request identities. Every transfer or transfer load whose
status is not `Done`, including failed or broken records, needs a disposition.
A `Done` load without a native `Done` transfer for the exact same network and
external transaction also requires a disposition and retained evidence: loading
and registration happen before financial completion.
An already `Done` transfer proves prior consumption and prevents another incoming
operation for the same network/transaction. A `Done` load alone does not prove
that the financial transfer completed.

The checkpoint has no native `Pending` transfer/load records in the current
independent inventory. Its failed transfer loads still need explicit evidence
dispositions. This observation does not establish the absence of unsubmitted or
post-checkpoint source deliveries. No production CSV is needed to implement the
gate; the final run uses the actual compiled replacement policy.

## Permanent mapping and the retained bridge halt

The current runtime-138 source resolves replacement beneficiaries after native
authentication even when recovery is `Complete`. Channel signatures still cover
the original commitment and recipient. Only the local application payload uses
the policy replacement; native source/token checks, message IDs and nonce/replay
rules remain in force. A malformed or mismatched persistent policy fails closed.
Ordinary channel application failure retains native semantics: the authenticated
nonce is consumed without a credit. Manifest catch-up has its separate outer
transactional rollback.

HASHI preserves the original signed `Requests` bytes. Transfer beneficiaries and
the query owners of all `Incoming`/`LoadIncoming` authors resolve independently;
an author is not evidence of financial ownership. Outgoing authors are unchanged.
Ordinary retained Pending loads follow `register_incoming_request` then
`finalize_incoming_request`; registration must preserve an already migrated load
index and must not recreate an old-owner index. Fresh imports and finalization
preflight policy resolution before request, reserve or cancellation mutations.
Permanent old-account retirement remains enforced.

This mapping support does not reopen a bridge. The release halt remains active
after runtime 138, and only finite manifest-approved catch-up can execute while
halted. Unmanifested or insufficient-evidence deliveries remain held. Reopening
still requires counterpart nonce/authorization reconciliation and the other
release evidence gates. Legacy maintenance runtimes 134–137 reject ordinary
persistent mapping once a recovery marker exists; their no-marker historical
path and weights are preserved.

The reviewed resolver patch is applied to source. On 2026-10-27, the v14 resolver
source cohort's native runtime suite passed with 364 tests and 5 ignored,
including all 15 new cases: seven HASHI, seven channel and one key-backed
retired-signer regression. The exit-zero receipt
is `artifacts/recovery-28191817/fixtures/resolver-native-verification-v14/runtime-full-attempt-1.json`;
its log records every case. Consult the current release's source- and
artifact-bound receipts for legacy/WIP tests, full Clippy, rebuilt recovery
artifacts and exact-Wasm gates; this source document does not certify those
statuses. The sealed v13 rehearsal certifies only its archived baseline cohort.
The diagnostics use disposable public
fixtures: HASHI covers fresh and retained Pending delivery, while the channel
diagnostic uses an unused Rococo transport under the separate mixed39 policy and
preserves real Liberland quarantine state. Neither proves production reopening,
external finality, or ownership of the synthetic fixture accounts. The channel
diagnostic does not establish TON, EVM or Parachain exact-Wasm coverage.

## Native HASHI deposits without a sender field

The native `Deposit(bytes32,uint256,address,bytes32)` event does not contain a
sender. Its existing decoder sets `IncomingTransfer.from` to zero. Recovery must
preserve that value, the original request bytes and the original transaction hash;
inserting a guessed sender would change the authenticated payload.

The appended `HashiAuthenticatedSource` work variant accepts this native form.
It binds a bounded, signed Ethereum execution transaction to the original
peer-authenticated transaction hash and verifies its signing domain. It recovers
the outer transaction actor for the explicit network attacker registry. This
does not establish an internal contract caller, depositor, beneficial owner,
receipt association or external finality. Ambiguous ownership still needs a
separate reconciliation disposition. The old `Hashi` encoding remains unchanged.

Native approval checks run on the original request. Only the effective local
beneficiary and query owner resolve through the recovery mapping. Credits and
exclusions consume the same native request/replay identity transactionally;
excluded actors receive no credit and create no issuance. Coverage verification
recognizes the new variant and rejects unsupported or malformed source proofs.

The v15 native checks passed 380 runtime tests, 39 node tests, 198 recovery-tool
tests and all three full Clippy modes. The parser and integration include
malformed-signature, wrong-domain, changed-payload and replay regressions. Their
receipts are in `artifacts/recovery-28191817/fixtures/hashi-source-native-verification-v15`.
These native results do not certify exact-Wasm execution. The new public fixture
has three separately authenticated operations: retained Pending credit, fresh
credit and a distinct excluded actor, with one identical registry throughout.
Its exact artifacts and execution receipts must be verified separately before
the current recovery candidate can pass the release gates.

## Bounded work transport

The `recoveryWorkIndex` artifact contains exactly:

```json
{
  "schema": "sora-nexus-recovery-work-index-v1",
  "policySha256": "SHA-256 of the complete raw policy.scale, without 0x",
  "work": [{"file": "work/00000.scale", "bytes": 100, "sha256": "actual SHA-256"}],
  "audits": [{"file": "audit/00000.scale", "bytes": 100, "sha256": "actual SHA-256"}],
  "terminal": {"file": "terminal.scale", "bytes": 100, "sha256": "actual SHA-256"}
}
```

Illustrative sizes and hashes must be replaced with actual values. Each file is
one complete `RecoveryInherent::encode_remark` envelope. Include every work slot,
every audit slot and the terminal audit, in policy order. A filtered incoming-only
list is rejected. The native verifier hashes each file, decodes its real action,
checks its cursor and recomputes its commitment against the corresponding policy
slot. It streams the files and retains incoming operations, touched storage keys
and preservation witnesses. JSON remains limited to 64 MiB, each shard to 576 KiB
and aggregate shard bytes to 512 MiB. Paths must be relative, unique and free of
symlinks or traversal.

The packager automatically includes every referenced shard in the bound release.
The index is transport, not an execution certificate. A fixture-only runtime
remains ineligible for production packaging regardless of its transport format.

## Typed audits and repairs after consumption

Each current-policy `Incoming` operation requires exactly one ordered
`IncomingAudit` with the same operation index and commitment. The compact audit
binds native request fields and exclusions. Native dispatch atomically stores a
bounded consumption companion and increments a protected pending-audit counter.
The audit checks receipts, mapped beneficiaries, native replay state and deferred
acknowledgements before marking the operation audited and decrementing the
counter. Terminal verification and runtime-138 admission require a present zero
counter; a missing counter is an error.

Request end timepoints are checked against the actual consuming block. HASHI
load submission heights follow native execution: fresh or eligible resubmitted
loads use that block; exact retained loads and finalization preserve their
recorded optional height. Fixture compilers replace only these exact dynamic
storage witnesses with typed checks. Static monetary witnesses remain. Tick or
repair delays cannot change the expected financial result.

A repair may audit a transfer already consumed under an earlier policy without
crediting it again. Checkpoint-only verification rejects `prior_policy` audits.
To admit them, add both arguments to the fixed command above:

```sh
  --recovery-replay /absolute/release/supplied-council-replay.json \
  --recovery-output /absolute/new-replay-output
```

The command executes the pinned recovery manifest from the verified checkpoint
in the same process. It captures earlier accepted incoming payloads, verifies
native/Wasm agreement, checks the companion against the real initialized audit
parent, and admits proof only after the audit succeeds under exact Wasm,
independent import and disk restoration. Successful activation is required.
An opaque in-process certificate joins the active policy, original lineage,
operation, counters, executing code and parent roots to the current audit.
Serialized reports or caller-supplied snapshots cannot construct that certificate.
The supplied-council path supports production inputs; public-council diagnostic
provenance remains explicit and is ineligible for production packaging.

This proves chain-local prior-consumption coverage. It does not establish
external finality, archive completeness, operator approval or overall readiness.
The outer release verifier must bind the supplied replay manifest and all nested
artifacts before using this path; the standalone CLI result does not replace
that packaging gate.

## External inventory and dispositions

The `incomingDeliveryInventory` artifact has this strict schema:

```json
{
  "schema": "sora-nexus-incoming-delivery-inventory-v1",
  "checkpointHash": "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0",
  "frozenHeadsSha256": "SHA-256 of externalFinalizedHeads, without 0x",
  "deliveries": [{
    "networkScale": "0x0200000000",
    "messageId": "0x plus the 32-byte external transaction/message ID",
    "evidenceHash": "0x plus BLAKE2b-256 of a bundled public evidence artifact",
    "originalBeneficiary": "original SORA Nexus AccountId32 or SS58 address",
    "obligation": "admit"
  }]
}
```

`admit` is an explicit reviewed execution obligation, never a verifier pass flag.
It must be discharged by an actual typed incoming work item or exact checkpoint
proof of prior consumption. It cannot become quarantine. `unresolved` records
have insufficient evidence and cannot be executed by this work; they need an
explicit evidence quarantine. A beneficiary may be `null` only when neither an
authenticated transfer payload nor the supplied source evidence establishes it.
An unresolved load's author is not automatically its financial beneficiary.

The `incomingReconciliation` artifact contains:

```json
{
  "schema": "sora-nexus-incoming-reconciliation-v1",
  "checkpointHash": "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0",
  "mappingCommitment": "0x plus BLAKE2b-256 of SCALE Vec<AccountReplacement>",
  "externalInventorySha256": "SHA-256 of incomingDeliveryInventory, without 0x",
  "decisions": [{
    "networkScale": "0x0200000000",
    "messageId": "0x plus the external transaction/message ID",
    "action": "process",
    "evidenceHash": "0x plus the bundled evidence artifact hash",
    "originalBeneficiary": "original account",
    "effectiveBeneficiary": "its policy replacement, or the same unaffected owner",
    "checkpointRecords": [],
    "quarantineReason": null
  }]
}
```

Exactly one decision must cover each identity in the union of unconsumed local
records, declared external deliveries and actual incoming work messages. Duplicate
or extraneous decisions fail. The mapping commitment is a data commitment that
does not introduce a circular Wasm hash.

The four actions have distinct effects:

- `process`: matches a typed work message, its source evidence hash and original
  recipient; the supplied runtime registries do not exclude that message.
- `exclude`: matches the same authenticated-path work structure, with exclusion
  selected by its source/recipient registry. Actual native authentication and
  atomic nonce/request consumption must still pass the exact runtime rehearsal.
- `retainConsumed`: proves an already `Done` native transfer, with no duplicate
  work, and retains its exact completion evidence through final audits.
- `quarantine`: preserves an unresolved obligation with no incoming work, credit,
  refund, nonce advance or completion claim. It requires one of `missingReceipt`,
  `unresolvedFinality`, `contradictoryEvidence`, `unsupportedLegacyAsset` or
  `unresolvedHistoricalError`. An old module-error index that cannot be resolved
  under checkpoint metadata is unresolved historical evidence, not a contradiction.

Each checkpoint record is bound by `key`, `valueHash`, `statusKey` and `statusHash`,
all canonical hexadecimal bytes/hashes. Include every non-`Done` record and every unresolved `Done` load for that
external identity. `retainConsumed` additionally includes the exact completed
transfer record. Quarantined and retained-consumed keys cannot be touched by a
storage work chunk, and their original value hashes must appear in final audit
witnesses. A changed historical prepared wrapper requires its own exact adapter;
the coverage verifier cannot reinterpret it as a fresh transfer.

The result exposes original/effective beneficiaries and each retained load
author's original/effective query owner. All referenced evidence hashes must name
actual files in the outer release bundle. Receipt hashes bind the policy, complete
work index, capture, external inventory, reconciliation and frozen-head file to
the originally admitted candidate. This verifies coverage of the supplied source
inventory; proving that inventory exhausts the frozen external chains remains a
separate mandatory release gate.

`compose-storage-work` derives exact preservation witnesses from the same native
checkpoint inventory used by the coverage verifier. It retains every unresolved
request/status pair by default, including an orphan `Done` load. The composer
checks the captured values, rejects conflicting storage edits, commits the
witnesses into its policy, and executes their audits in its storage rehearsal.
Its `incomingCheckpointPreservation` report is a source-preservation statement;
it does not claim incoming authentication, execution or external completeness.

Processing a preserved checkpoint request later requires authenticated native
execution and rederivation of the final audit values in the complete work plan.
Silently removing these witnesses is not a valid way to admit catch-up. New
external deliveries without checkpoint request records still require the same
external evidence, typed operation, nonce/status checks and coverage disposition.


A later forward repair can omit an incoming audit that already succeeded under
an earlier policy. Coverage imports that operation only from the same-process
replay certificate, after rechecking the permanent audit marker, original
consumption companion, native nonce/status, full request, receipt and deferred
acknowledgement against the final independently imported and restored state.
The original audited policy and its accepted counter remain distinct from the
final policy. This read-only inspection never repeats the on-chain audit or
changes its cursor/pending count. Final pending audits must be zero; changed
beneficiaries, consumption records, counters or final-policy identity reject the
certificate. Final-state roots and validation witnesses are diagnostic bindings,
not caller-provided authorization. Full/warp finality, external-finality and
operator-approval gates remain separate.
The maintenance report exposes `incomingReplayBinding` only when it contains
these authenticated audit proofs; it is copied from the same completed private
certificate. Package coverage requires that binding to equal its certificate
binding exactly. Neither serialized copy can create or replace the certificate.
