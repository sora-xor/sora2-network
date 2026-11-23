# SORA Nexus read-only HASHI live-client adapter

`node/src/recovery_hashi.rs` is attached to the existing live client by the
optional local-file processor in `recovery_hashi_local.rs`. It has focused source
tests. The adapter adds no RPC method or signing/coordinator protocol. Formatting
has been checked; this document does not claim that the new host module has
compiled or executed against a release Wasm.

The adapter attaches to the existing admitted `FullClient`. It creates no
network, transaction pool, telemetry task, offchain worker, keystore or HTTP
client. `prepare` obtains native signing payload bytes; `admit` checks an already
signed operation through the existing native incoming path. Neither function
signs, submits, persists storage, advances a cursor or produces a block. Peer
authentication and external-source verification remain the existing HASHI
peers' responsibility.

## Trusted attachment boundary

The service constructs a private admitted-client token with the **same
Configuration used to construct that client** and an independently admitted
release-manifest SHA-256 pin. Attachment repeats recovery configuration/database validation,
including the prohibition on runtime overrides. It verifies the manifest's
checkpoint/policy identity and exact artifact sizes, SHA-256 and Blake2 hashes
for policy, bootstrap Wasm, execution plan and bootstrap metadata. The original
spec-134 / transaction-131 bootstrap corridor is supported. An ordinary release
may carry `incomingRecoveryReplay` evidence; that field is not treated as a
repair marker. Exact configuration/policy bytes and active-policy storage exclude
a revised-policy or repair-runtime corridor.

The expected outer manifest hash must come from release admission, never from a
request that also supplies its own manifest. This adapter validates the selected
artifact bindings; it does not replace the release package's full evidence and
rehearsal gates. Construction is not a generic request-deserialized interface.
The policy commits the native operation's business fields while excluding the
future signature witness bytes, as in the existing runtime implementation.

The execution plan uses the existing
`sora-nexus-hashi-execution-plan-v1` schema. Both absolute execution-block bounds
are immutable release artifacts. Every encoded mortal era must have one birth
and death across that entire interval. A later mortal cycle cannot silently
rebase a previously approved template.

## Per-call state and capability checks

The requested parent must equal the client's current finalized hash. The best
head may be ahead. Recovery lineage, checkpoint import provenance, original and
active policy, Applying phase, exact cursor, operation counter zero, bridge halt,
absence of transient context and the recorded first-child anchor are checked.
The anchor's actual header must be available. The selected child's height must
be inside the immutable execution interval.

`FullClient::code_at(parent)` verifies the actual substitute bytes; stored
`:code` is recorded separately. `code_at` ignores overrides, which is why the
same-configuration attachment and override prohibition are essential. Runtime
metadata is obtained from the exact executing code with the fixed
`Metadata_metadata` method and matched to the release artifact by hash. No
spec-version metadata cache is used.

The only other methods are `EthBridgeRuntimeApi_recovery_hashi_prepare` and
`EthBridgeRuntimeApi_recovery_hashi_admit`. Every method uses
`CallExecutor::contextual_call`, a fresh disposable `OverlayedChanges`,
`CallContext::Onchain` and an explicitly empty `Extensions::new()`. Unlike the
executor's ordinary `call`, this SDK path does not obtain the client's global
extension factory. The adapter therefore supplies no signing, HTTP,
transaction-pool or offchain-database capability, even on a running authority
client. The audited runtime API performs native work inside rollback. Any
remaining top/child/offchain write or open storage transaction rejects the
result; the overlay is always dropped.

SCALE input/response decoding is memory-bounded and rejects trailing bytes.
Responses must match the actual parent/root, anchor, policy, operation commitment,
network, version, stored-code hash, source-evidence commitment and ordered signer
call/extra hashes, nonces and absolute birth/death bounds. Birth headers must be
locally available. Native admission must report the exact incremented cursor and
correct Applying/Auditing transition with no persistent-state or external-finality
claim. The finalized parent and executing code are checked again after the call;
if finality advances, the caller must obtain a fresh result.

These results establish local runtime admission at the node's own finalized
state. They are not an independently exported GRANDPA proof, Ethereum beacon
proof, full-block execution rehearsal, authorization to sign, or bridge credit.
The existing separate finality and full-block release gates still apply.

## Opt-in local request processing

Start the upgraded node once with both options:

```
--recovery-hashi-witness-dir /absolute/canonical/public-witness-directory
--recovery-hashi-release-sha256 0x<independently-admitted-manifest-sha256>
```

Both options are absent by default and require each other. The directory and its
`requests/` and `results/` subdirectories must already exist. Keep it separate
from secret material. Files must be regular, bounded and unaliased; relative
references cannot escape the directory. On Unix hard-linked inputs are rejected.
The existing service/client remains live; no database copy or per-request restart
is needed. This processor has no access to a signer or transaction pool.

`release-input.json` has these exact fields; each reference is
`{"file":"relative/path","bytes":123,"sha256":"64 lowercase hex"}`:

```json
{
  "schema": "sora-nexus-hashi-local-release-v1",
  "release": {"file":"release.json","bytes":123,"sha256":"<sha256>"},
  "bootstrap": {"file":"bootstrap.wasm","bytes":123,"sha256":"<sha256>"},
  "metadata": {"file":"bootstrap-metadata.scale","bytes":123,"sha256":"<sha256>"},
  "metadataRole": "bootstrapMetadata"
}
```

Publish each complete request by atomic rename from outside `requests/` to
`requests/<SHA256-of-exact-request-bytes>.json`. The request has exact fields:

```json
{
  "schema": "sora-nexus-hashi-local-request-v1",
  "attemptId": "0x<nonzero-32-byte-public-attempt-id>",
  "action": "prepare",
  "parentHash": "0x<current-finalized-recovery-parent>",
  "planRole": "hashiPlan0",
  "plan": {"file":"plans/0.json","bytes":123,"sha256":"<sha256>"},
  "operation": {"file":"operations/0.scale","bytes":123,"sha256":"<sha256>"}
}
```

`operation` is the native encoded `IncomingOperation`, not its outer inherent
or an arbitrary call. `action` is exactly `prepare` or `admit`. The plan role
must name the exact plan in the immutable manifest. One directory supports many
release-listed plans/work cursors. Signature bytes may change between preparation
and admission; all policy-committed business fields must remain identical.

One blocking task checks for new public requests every two seconds and processes
at most one request per iteration. It bounds the directory to 4,096 entries;
archive completed request files outside the queue if necessary, preserving their
results. Each attempt exclusively creates `results/<request-sha256>/` before
execution. A successful `result.json` binds its raw `response.scale`, operation,
plan and actual host identities. The SCALE file is the successful native return
structure, without an outer Result tag. An error creates an explicit failed
receipt. An interrupted attempt retains `started.json`; existing attempt
directories are never retried or overwritten. The caller can intentionally make
a new attempt with a new public `attemptId`, after correcting the cause. There
is no automatic retry, rebase or re-sign operation.

A finalized-parent change causes failure; the coordinator must wait for its
chosen finalized parent and produce a new request. Best-head movement alone is
allowed. Request and startup artifact hashes are checked again after execution.
The processor writes only its separate results and never populates the existing
block-authoring inherent directory. A successful response authorizes neither
signing nor submission: existing HASHI peers still perform their existing source
checks and signature workflow.

## Remaining verification

The node manifest uses the already pinned local `eth-bridge-runtime-api`,
`sp-externalities` at `polkadot-stable2609`, and `serde` derive dependencies. No
runtime API ID/version or transaction authentication change is needed. The
service owns the admitted token and supplies its actual Configuration; requests
cannot construct that token or add runtime extensions.

The 13 focused adapter tests and the full 62-test node suite passed, as did
strict node Clippy with production feature extensions disabled. The tests cover
artifact tampering/role collisions, original-policy replay evidence and revised
policy rejection, signature-witness freedom with immutable business fields,
absolute-era boundaries, empty capability construction, storage/offchain/open
transaction rejection, response-context drift, file/hash/alias guards, exclusive
outputs, and interruption markers. They do not claim to exercise a complete live
client. The exact bootstrap Wasm must also be run through the concrete adapter
on a real finalized recovery parent, including a best head ahead of finality,
stale-finalized-parent rejection, actual metadata identity, native successful and
failed signature admission, and proof that client state/cursor remain unchanged.
Retain those actual execution receipts before enabling an existing peer
coordinator to consume its output.
