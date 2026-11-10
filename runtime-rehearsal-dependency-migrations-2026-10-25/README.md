# SORA Nexus production-state rehearsal

This new evidence directory preserves the archived 4.8.12 release. Public RPC is read-only and pinned to a finalized block. Candidate runtime execution and recorded synthetic fixtures run locally with mocked outer transaction/header signatures. No private keys, signed public submissions, governance enactment, or deployment are performed.

The final current-source production-feature Wasm was executed with SHA-256
`2109096d15dd84e49c7d081ad20ab7b194424cc4e640f804133da0b836c28adf`
(1,900,414 bytes; spec 135 / transaction 131). The secure dependency manifests, canonical Linux-built native assets, script provenance, snapshot code/metadata, and all outputs are recorded separately here.

Baseline capture: finalized block 28,179,122, hash
`0x32fd0d8dd97744f86dcf9d964d7fcd25fea43ab5e6d9d7054db9b7b75eba8cd0`.
Production is spec 134 / transaction 131 and runs archived 4.8.12 code
`de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221`.
The proposed current-source candidate must be spec 135 (or an explicitly
reviewed later version), preserve transaction version and all existing SCALE/API
contracts, and strictly advance this baseline. The copied verifier enforces this
before local fork execution. The existing-target diagnostic is explicitly not
final build evidence.

The new package manifest/lock is an active sixth npm graph. CI audits it and
requires the same canonical local package pins as the other maintained graphs.
Its fresh audit has zero findings across 90 dependencies. Updates remain deliberate
and must preserve the regression fixtures.

To reproduce the run and its independent raw-admission check:

```sh
REHEARSAL_NODE=/path/to/node24 CANDIDATE_SPEC_VERSION=135 \
  ./run-local-rehearsals.sh /absolute/path/to/final-candidate.compact.compressed.wasm \
  /absolute/path/to/candidate-build-evidence.json
/path/to/node24 validation/verify-rejected-admission.cjs
```

The runner binds the exact artifact to supplied build evidence (candidate hash,
actual build command and source input hashes). It preserves previous reports and
a changed earlier candidate under `validation/attempts/`, then creates a new
`validation/rehearsal-run.json` with a unique run ID. A failure replaces the current
run status; no old passing stage report can satisfy a new run. Public code/state/metadata is pinned; synthetic overrides and mocked
outer signatures are recorded by the fixture reports. The native migration
rehearsal is prepared separately and uses the same finalized block. Current
migration markers are already applied, so this production replay tests idempotent
paths; active historical migrations require separate fixture coverage.

The `validation/attempts/` directory preserves obsolete capture gates and
superseded rehearsal attempts; its README identifies the invalid-lifecycle run. `validation/negative-checks/` contains deliberate candidate-version rejection
runs, summarized as passed tests in `version-gate-negative-checks.json`. Neither
is presented as a failed final candidate run. Final required gates use the main
validation report paths.

The compatibility gate compares function signatures for both host imports and
exported runtime APIs, independently of compiler-internal index ordering. Its nine
offline Wasm fixtures verify signature changes and memory bounds. The current
static heap strategy must fit the wasm32 address space; this source/bound check
is separate from the required actual candidate execution.

The pinned bridge inventory completed all 2,061 pending operations in 17 pages,
with no truncation at the 10,000-operation safety bound. Every operation was
hash-only (no stored `Calls` payload), so no payload length can be certified from
that state. The three current peers and source-derived per-proposer quota are
recorded; this public observation is not operator readiness attestation.

Independent fee-event verification binds candidate metadata to the passing Wasm
compatibility report and cross-checks candidate, deployed code, snapshot and
rehearsal hashes. Five negative binding tests and six run-status/build-proof tests cover
changed inputs, stale successes and Python optimization; optimized Python is
rejected. Three provider-level tests block submission/administrative methods.
The bridge layout is extracted from the exact captured deployed Wasm, and the
full-inventory gate fails if pagination reaches its cap without completeness.

The October 25 security refresh is in `validation/npm-security-refresh/`:
all six audited dependency graphs have zero findings. An isolated installed copy
passes 7 integration, 6 schema and 3 Blob tests, 2,619 crypto assertions across
all 32 exports, and the executor CJS/ESM smoke suite. Both that copy and the local
rehearsal install match all 32 canonical native asset hashes. The isolated copy
avoids unrelated packages in the developer's ancestor `node_modules`; those
external packages were not changed. No native rebuild was needed for this refresh.

Build evidence is accepted only after the full final build report and its required
gates pass. Fingerprint, source-check and log hashes are validated; the full source
inventory must remain unchanged both before and after the local rehearsals.

Final local run `20261024T174836Z-fa72028d` passed every required stage and the
full 4,542-file source check before and after execution. The source aggregate is
`e7baceebe77829ca94a2a8c79f1a83704b759582570f74836c9d0607321dfb69`.
`validation/rehearsal-summary.json` binds the final build, metadata, runtime
execution, bridge inventory and refreshed dependency evidence.

The final suites cover 9 signed equivocation cases and 4 unsigned methods,
117 signed policy cases and 8 unsigned methods, and 31 payout cases. Independent
fee-event verification checked all 9 equivocation debits. The postrun raw-admission
verifier checked 22 invalid calls and 52 discarded writes: every write was to
`System.InherentsApplied`, `System.BlockWeight` or `System.ExtrinsicData`; none
changed accounts, fees, rewards, claims or events. Three negative tests ensure
that verifier rejects other storage writes and malformed accounting keys.

The upgrade block explicitly returned `OnlyInherents`. Both suites applied only
timestamp, finalized that block through candidate Wasm, recorded migration
completion and a cleared cursor, then linked and initialized the next block with
`AllExtrinsics` before any signed fixture. Five offline tests enforce inclusion
mode and block-builder rollback semantics. Invalid transaction raw diffs are
recorded and discarded; admitted dispatch errors commit their fee/nonce effects.
The one removed export, `__data_end`, is an unused linker global; inspected hosts
require preserved `__heap_base`, and every runtime function signature remains
exact. The final candidate executed successfully with this export delta.

This remains local execution against a provider-trusted snapshot with mocked
outer signatures and 306 disclosed storage override records. Partial-backend
state roots, consensus seals, real user payouts and operator activation readiness
are not certified. Native migration and Rust regression reports are separate.
