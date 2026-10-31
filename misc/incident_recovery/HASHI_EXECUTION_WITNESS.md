# SORA Nexus offline HASHI execution witnesses

`hashi_execution_witness.py` appends public signature bytes to a separate local
evidence directory. It does not sign, submit an inherent, read private keys,
contact a peer or execute a runtime. It supports the original recovery policy
under bootstrap runtime 134 / transaction version 131 only. Forward-repair
policies need a separately reviewed extension.

The immutable release must already bind the policy, bootstrap Wasm, executing
metadata, exact HASHI template, baseline envelope and an execution plan. The
helper checks their byte lengths, SHA-256 and Blake2-256 hashes against release
artifact roles. The template's operation commitment must occupy its exact work
index in the decoded policy. It reuses `hashi_codec.py` for both HASHI variants,
native SignedExtra, header ancestry, signing payloads and SCALE encoding.

The baseline and new envelope are independently encoded from that same template.
Only the approval `MultiSignature` bytes may differ. A changed signer, call,
nonce, era, request, source transaction, network registry or evidence commitment
requires new policy work; this helper cannot amend the release.

The input manifest has schema `sora-nexus-hashi-execution-witness-input-v1`, an
exact `files` object and `releaseRoles`. Every file reference is
`{"file":"relative/path","bytes":123,"sha256":"64 lowercase hex digits"}`.
Paths must remain within the input directory without symlinks or aliases. The
eleven file roles are:

| File role | Content |
| --- | --- |
| `release` | Immutable `sora-nexus-recovery-release-v1` manifest. This helper checks the relevant bindings; it does not replace full release admission. |
| `policy` | Original canonical policy SCALE, bound by release role `policyScale`. |
| `wasm` | Bootstrap Wasm, bound by release role `bootstrapWasm`. |
| `metadata` | Executing metadata bytes. |
| `template` | Existing HASHI template JSON without signatures. |
| `plan` | Immutable execution window and runtime identity, described below. |
| `baseline` | Release-bound complete Incoming recovery envelope. |
| `context` | Exact observed parent, lineage, runtime and nonce context. |
| `signatures` | Public signature witnesses bound to the context and template. |
| `stateEvidence` | Retained evidence bytes for the supplied state observations. |
| `finalityEvidence` | Retained finality evidence bytes referenced by the lineage. |

`releaseRoles` maps `template`, `plan`, `baseline` and `metadata` to their distinct
existing release artifact roles. It cannot rename `policyScale` or
`bootstrapWasm`. A package lacking these bound artifacts is not supported; do not
insert them into an already sealed manifest.

The plan schema is `sora-nexus-hashi-execution-plan-v1`. Its exact fields are
`schema`, `templateCommitment`, `network`, `firstExecutionBlock`,
`lastExecutionBlock`, `specVersion`, `transactionVersion`, `executionCodeHash`
and `metadataHash`. Both ends of the finite execution window must use the same
committed mortal birth block for every signer and precede that era's death.
Network numbers are native HASHI network identifiers, not EVM chain IDs.

The context schema is `sora-nexus-hashi-execution-context-v1`. Its exact fields
are `schema`, `releaseManifestSha256`, `templateCommitment`, `network`,
`parentHash`, `parentNumber`, `signingContext` and `stateObservation`.
`signingContext` uses the existing codec schema documented in `HASHI_CODEC.md`.
Its contiguous headers must end at the exact parent of the intended execution
block, and their policy digests must match the original release. Its runtime and
metadata hashes must match the plan. Every required mortal birth header must
occur in the supplied ancestry after the checkpoint.

`stateObservation` contains only `at`, `policyCommitment`, `phase`, `cursor`,
`operationCounter`, `accountNonces` and `evidenceSha256`. The expected phase is
`Applying`, cursor is the template index and operation counter is zero.
`accountNonces` is the ordered list of `{signer,nonce}` for every committed
approval. Both its parent and evidence digest must match the supplied files.
These are consistency checks on observations, **not state-proof verification**.

The signature file schema is `sora-nexus-hashi-signature-witnesses-v1`, with
exact fields `schema`, `contextSha256`, `templateCommitment`,
`baselineSignatures` and `signatures`. Both signature lists follow the template's
approval order and use canonical SCALE `MultiSignature` encodings. Baseline
signatures must reconstruct the original envelope exactly. New signatures cannot
be zero placeholders. Their cryptographic validity is deliberately left to
native admission; nonzero bytes alone do not authenticate a peer.

After the operational coordinator has collected these public files, invoke:

```sh
python3 misc/incident_recovery/hashi_execution_witness.py \
  --manifest /absolute/path/to/input-manifest.json \
  --manifest-sha256 EXACT_SHA256_OF_THAT_MANIFEST \
  --journal /absolute/path/to/execution-witnesses
```

The helper exclusively creates a subdirectory identified by the release-manifest
hash and operation index. It preserves all inputs, the input manifest, exact
`envelope.scale`, `bundle.json` and a completion file containing the report hash.
An existing directory, including a partial attempt, is never overwritten or
deleted. Retrying the same operation in that journal fails. This local append
rule is **not chain replay protection**; copying the inputs to another journal
does not make them safe to execute twice. Native nonce and request-consumption
checks remain authoritative.

All bundles keep signature verification, native admission, state-proof
verification, finality, runtime execution, chain replay protection and production
readiness false. In particular, a hash of caller-supplied finality evidence does
not authenticate the named head. A fixed native verifier must independently join
the actual release, authenticated parent state, nonce, membership, request and
signature results before the envelope can be supplied to a recovery node. Keep
that result as separate evidence; never rewrite this bundle to claim it happened.

`unverifiedSigningPayloads` (including their birth hashes) and
`unverifiedEraDeaths` are derived from the supplied, unauthenticated headers.
Each peer must derive and compare its payload against its own node's finalized
recovery chain before signing. The bundle is not a trusted signing request.

Operational peer signing and fixed native bundle admission remain pending work.
Do not resume outbound workers to obtain these signatures. A changed nonce or
expired immutable window needs the existing authorized repair path. The pure
tests use synthetic public fixtures and never certify production signatures.

```sh
cd misc/incident_recovery
python3 -B -m unittest -v test_hashi_execution_witness.py test_hashi_codec.py
```
