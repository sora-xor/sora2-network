# SORA Nexus recovery fixture artifacts

These tools prepare isolated public fixtures while production substitutions,
operator approvals and signing-stop evidence are still pending. **Never deploy
fixture artifacts.** Their policies may cancel actual checkpoint bookkeeping in
a local replay, but they do not authorize a production state transformation.
Their GRANDPA keys are deterministic, publicly derivable test keys.

The native bootstrap tests cover maintenance author/import state-root agreement,
periodic epoch progression with frozen sessions, persisted-trie restart between
chunks, committed failure, strict-majority escape, forward repair and replay
rejection, and automatic activation admission. Native code-identity tests use an
explicit runtime-version host fixture. They do not establish Wasm compatibility,
first-child checkpoint admission, BABE seals, GRANDPA finality or full/warp sync.
Those require the exact compiled artifacts and separate executable rehearsals.

`fixture_artifacts.py final-source` exports the current tracked and untracked
build sources into a **new** directory. It preserves Cargo.lock and the root
working tree, records every input file hash, detects concurrent source changes,
and writes the fixture policy only into the export's generated source. A policy
input can be raw SCALE or the native cancellation rehearsal's JSON containing
`work.policyScale`.

```sh
python3 misc/incident_recovery/fixture_artifacts.py final-source \
  --policy artifacts/recovery-28191817/checkpoint-bridge-cancellation-rehearsal.json \
  --destination /private/tmp/sora-nexus-recovery-final-fixture
```

Build this export with the fixture runner. It uses the pinned toolchain, LLVM 22,
canonical workspace hint, offline locked Cargo and the source directory's existing
`target`. It strips inherited build-skip controls, records the actual logs/exit,
checks the embedded runtime identity, and retains the source, lock and artifact
hashes. It does not run Cargo for source preparation alone.

```sh
python3 misc/incident_recovery/build_fixture_wasm.py \
  --source /private/tmp/sora-nexus-recovery-final-fixture \
  --output artifacts/recovery-28191817/fixtures/builds-v1/final-138
```

For a later final-runtime fixture, use the explicit refresh helper. It verifies
the previous source hashes, archives the complete previous source and provenance,
and preserves the warm target. It refuses local source edits, unknown files,
concurrent builds, production exports and bound legacy substitutes. Supply new
evidence and build-output directories on every revision.

```sh
python3 misc/incident_recovery/refresh_fixture_source.py \
  --source /private/tmp/sora-nexus-recovery-final-fixture \
  --policy artifacts/recovery-28191817/checkpoint-bridge-cancellation-rehearsal.json \
  --evidence artifacts/recovery-28191817/fixtures/source-refresh-v2
```

Keep the root `runtime/src/incident_recovery/generated.rs` unbound until release.

The observed checkpoint inventory has no operator-approval semantics. For local
execution tests only, generate complete test operator records from the verified
version-2 inventory:

```sh
python3 misc/incident_recovery/fixture_artifacts.py operators \
  --inventory artifacts/recovery-28191817/checkpoint-consensus-membership-v2.json \
  --destination artifacts/recovery-28191817/fixtures/operators-v2
```

Each record preserves the exact observed active/queued/NextKeys signing-key
unions and assigns a deterministic fresh Ed25519 test public key. No keystore or
production credential is read, and no private key file is written. The output
explicitly says operator approval and signing-stop evidence are unverified.

After building final 138, create hold-135 bindings using the local fixture's
checkpoint GRANDPA set and first-slot values. These slot values are execution
inputs only; production needs a slot beyond every reported abandoned-chain
signature.

```sh
python3 misc/incident_recovery/fixture_artifacts.py bindings \
  --policy /absolute/path/to/policy.scale \
  --operators /absolute/path/to/operators.scale \
  --final /absolute/path/to/final-138.wasm \
  --previous-set LOCAL_FIXTURE_SET \
  --first-slot LOCAL_FIXTURE_SLOT \
  --destination /absolute/new/hold-binding-directory
python3 misc/incident_recovery/bootstrap/prepare.py \
  --spec-version 135 \
  --binding /absolute/new/hold-binding-directory/hold-135-binding.json \
  --destination /private/tmp/sora-nexus-recovery-hold-fixture
```

Build hold 135 with the same runner in its own persistent lane. If the reviewed
unbound legacy lock already adds only the runtime's `sp-trie`/`sp-externalities`
direct edges, pass it with `--resolved-legacy-lock`. The runner rejects any
package identity or other dependency drift and records both original and
resolved lock hashes. The later Cargo invocation remains `--offline --locked`.
For sequential legacy fixture builds, `--target-dir` can reuse the canonical
unbound development target. The runner takes a target-level exclusive lock in
addition to the source lock, records the selected path, quarantines stale Wasm
exports there, and keeps `WASM_BUILD_WORKSPACE_HINT` bound to the actual source.
Never run an independent Cargo command concurrently in that shared lane.

Then repeat `bindings` into another new directory with
`--hold /absolute/path/to/hold-135.wasm`. It additionally emits the bootstrap-134
binding. Export a new bound bootstrap source lane with `prepare.py --spec-version
134 --binding .../bootstrap-134-binding.json`. Never overwrite an existing bound
export or its immutable bootstrap artifact.

Both binding directories and all source exports carry fixture provenance.
The legacy preparer propagates the adjacent fixture marker to its own provenance;
the production package verifier rejects fixture provenance. Hash and retain the
actual final/hold/bootstrap bytes before running the maintenance Wasm harness.
Any change to bound source or public inputs produces a new fixture artifact and
requires its own rehearsals. It cannot replace an already published substitute.

## Separate public council escape rehearsal

`prepare_escape_fixture.py` creates an **escape-only** fixture with three public,
deterministically derived Ed25519 test council keys. They are not the observed
checkpoint council and never authorize production governance. The harness checks
that actual `Council.Members` remains byte-for-byte unchanged throughout this
fixture. Its sole non-consensus write is a named public probe key. No financial
state transformation or final activation is permitted.

The first committed probe deliberately has an incorrect old-value hash, taking
the actual bootstrap into `Failed`. The harness reads the persisted status and
actual stored/executing code, derives and signs the complete repair authorization,
and executes 134→135. It stages the exact compiled 136 bytes, signs a forward
repair from the observed hold state, then executes the corrected probe and its
storage audit. Every block still requires independent Wasm import and fresh disk
reconstruction. Insufficient quorum, duplicate signers, changed context/code
fields, changed signing domain and old signed envelopes are negative cases.
Signed messages, SCALE authorizations, context records and recovery envelopes are
saved beside the block evidence; they are not guessed by the Python generator.

Use `inputs` to prepare original/revised policies and hold135/repair136 source
bindings. Build those two independent artifacts sequentially in the warm legacy
target, then use `bootstrap-binding` with the exact hold bytes and build134.
Finally `manifest` binds the three artifacts and public consensus fixture inputs.
The manifest has a distinct public-fixture schema, as does the resulting witness.

An existing runtime138 may supply the required, version-checked final-code binding
for this fixture, but it is **unused** and its compiled policy need not match the
test council. No terminal audit is supplied, no final code is staged, and the
harness rejects final staging and activation actions. This fixture supplements the
separate automatic-activation rehearsal; it cannot replace that rehearsal or a
production quorum/ownership gate.

## Complete dynamic-operation write expectations

A rehearsal containing `Incoming` or `Outgoing` now requires `fixtureExpectations`, an outer-manifest `Artifact` pointing to a JSON document with schema `sora-nexus-public-fixture-storage-expectations-v1`, `fixtureOnly: true`, the exact `policyCommitment`, sorted `records: [{key, valueHash}]`, and `operations: [{index, records}]`. Null hashes mean actual absence, distinct from empty values. This is a public diagnostic fixture interface; it does not confer production readiness or external-finality approval.

Global records are checked from the actual persisted and reconstructed trie after every block once work reaches Auditing, Ready or Complete, including final activation and ordinary blocks. A value may be up to 32 MiB, with 64 MiB aggregate reads, so retained opaque preimages can be checked independently without a new runtime action. Records containing the policy commitment, such as incoming receipts and outgoing tombstones, belong here to avoid a circular policy hash.

Each operation record set must be the complete native-preview before/after difference at that committed work index. The exact-Wasm harness compares its finalized non-consensus write set, including `System.Account`, recovery status, receipts, tombstones and acknowledgements, to those key/hash pairs. It rejects extra writes, missing writes, changed values and any child-storage mutation. Only the System/BABE/GRANDPA/Session/Timestamp bookkeeping namespaces, excluding System.Account, plus the exact extrinsic-index and maintenance BABE-interval keys are outside this comparison. No broad recovery namespace exemption is allowed. Native preview is evidence used to construct the expectation; only the separate author/import/restart Wasm execution can establish that the artifacts obey it.

Older immutable automatic-v2 and escape-v1 runs contain no dynamic bridge operations. Their manifests and evidence remain unchanged and do not certify newer source changes.

## Public HASHI signature witnesses

The `bridge-fixture` utility feature additionally provides
`prepare-hashi-bridge-fixture --capture CHECKPOINT_DIRECTORY`. It requires the
explicit native test binary and SHA-256 pins used by the mixed bridge exporter.
Its separate cfg(test) exporter is
`incident_recovery_hashi_preview_tests::export_public_hashi_preview`.

This fixture uses the observed bridge account and registered sidechain DAI,
temporarily installs four deterministic public Ed25519 peers, and commits one
transfer whose author and beneficiary both use the synthetic replaced account.
It creates no setup funds. Native preview must show exactly one base unit minted
to the replacement, exact request and replay identity, and no old account index.
Temporary peers, membership and their provider/nonce accounts are restored by a
committed guarded chunk. The observed bridge asset configuration and permissions
are retained. Two native previews bind stable audit/issuance projections and then
the final policy-dependent receipt hashes; they do not constitute Wasm evidence.

Pass its work report to `prepare_maintenance_fixture.py`. The generated manifest
uses `sora-nexus-public-fixture-hashi-replay-v1`, pins explicit public peer
provenance in `fixtureHashiPeers`, and sets `fixtureHashi: true` only on the
committed incoming step. The ordinary rehearsal schema cannot enable this
signer, and a utility built without `bridge-fixture` rejects the schema.

Public keys derive from SHA-256 of
`sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-peer/v1/` followed by the decimal index
0 through 3. No supplied secret, keystore or production signature is read.
After the actual Wasm initializes the block, the harness checks persisted policy,
phase/cursor, peer membership, nonce, mortal era, genesis and executing runtime
version. It signs only the pinned native import call, using the actual stored
post-checkpoint birth hash and matching authored header/lineage. It cannot alter
business-call fields or the operation commitment. Full signing preimages,
payloads, signatures and context are retained as diagnostic evidence.

Invalid-signature and wrong checkpoint-birth-signature variants must be rejected by the
actual Wasm without changing state. The accepted receipt is replayed and must
also reject. Independent import, disk restart and complete dynamic write-set
checks remain mandatory, followed by automatic activation and ordinary blocks.
The native preview's explicitly synthetic birth hash never substitutes for this
actual-header signature verification. Every artifact remains **PUBLIC FIXTURE
ONLY — NEVER DEPLOY**, regardless of successful execution.
