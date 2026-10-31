# SORA Nexus stable2609 production-state rehearsal

This directory contains a separate rehearsal of the SDK migration candidate,
using the exact production Wasm artifact and a pinned read-only production
snapshot. The predecessor is spec 134 / transaction 131 at finalized block
28,179,122 (`0x32fd0d8dd97744f86dcf9d964d7fcd25fea43ab5e6d9d7054db9b7b75eba8cd0`).
Public state queries use `wss://mof2.sora.org`; execution remains local.

The verified candidate is spec **136**, transaction **132**. The new leading
`AuthorizeCall` extension supports the upstream heartbeat authorization migration.
Its explicit and implicit encoding are both empty. The compatibility gate checks
that exact exception and preserves the existing seven extensions and their
order, types, and signed payload layouts. Upstream `System.CodeUpdated` adds
the runtime code hash to its existing event payload; its SCALE encoding changes
and clients must refresh metadata. `Offences.Offence.timeslot` is renamed `slot`
with unchanged field encoding. The comparison reports the raw differences and
verifies these precise exceptions before checking every remaining interface.
Identity adds `remove_registrar` at call index 24 with a compact `u32` argument,
and `RegistrarRemoved` at event index 22 with a `u32` payload. The event is
appended locally to preserve the 22 deployed event indices. The gate validates
those exact additions and rejects other metadata and host/API differences.

Final verification passed on 2026-10-25: all three Clippy modes, 881 Rust test
executions, the fresh native production-state replay and all 18 exact-Wasm stages.
The three existing ignored tests remain documented. The complete hash-bound
index is `validation/rehearsal-summary.json`; the human-readable report is
`../misc/runtime_upgrade/sdk_migration_2609_2026-10-25.md`. This is local
verification, with no production transaction or deployment. Earlier release and
dependency-migration evidence remains separate.

```sh
REHEARSAL_NODE=/path/to/node24 CANDIDATE_SPEC_VERSION=136 \
  REHEARSAL_SIGNING_PYTHON=/path/to/verified-python-environment/bin/python \
  ./run-local-rehearsals.sh /absolute/path/to/candidate.wasm \
  /absolute/path/to/candidate-build-evidence.json
/path/to/node24 validation/verify-rejected-admission.cjs
```

The runner verifies full source identity before and after execution, all build
and tool hashes, the pinned baseline code, finalized upgrade-block lifecycle,
fee/reward/storage preservation, rejected-call rollback and bridge bounds.
Synthetic account/state fixtures and mocked outer signatures are disclosed in
individual reports. These are not consensus proofs or public transactions.
The signing gate independently reconstructs eight governance payload/extrinsic
cases from exact candidate SCALE metadata, verifies synthetic signatures, and
rejects the predecessor transaction version with network calls disabled.
The fee lane has a 45-minute watchdog, bounded at one hour for explicit overrides;
every behavioral assertion remains mandatory. Prior timed-out diagnostic output
and interleaved old/candidate API timing evidence are retained separately.

The copied immutable public read cache is keyed by block hash and is an
independent local copy. Native replay produces a fresh scoped snapshot. These
large reproducible state files are Git-ignored; result reports bind their hashes.
Bridge operator readiness is separate from local runtime validation. This lane
uses no production keys and performs no public submission or production
deployment. Deterministic synthetic keys sign local heartbeat and migration
fixtures so their inner authentication executes in the real Wasm runtime.

The fresh dependency check in `validation/npm-security-refresh/summary.json`
records seven isolated npm graphs with zero vulnerabilities on 2026-10-25.
`installed-checks.json` binds the successful clean install, canonical executor
and cryptography source/asset verification, and installed integration tests.
Those checks reuse the previously built native bundles; the new candidate's
host compatibility and runtime behavior still require the separate Wasm gates.

`validation/verify-sdk-patches.py` independently reconstructs all seven vendored
primitives/builder packages from committed Git objects plus their recorded Rust
patches and manifest overrides. It compares the complete Rust file inventory,
including materialized SDK documentation. The runner repeats this check and five
negative provenance tests; set `SDK2609_UPSTREAM` to an existing SDK clone that
contains commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe` when rerunning elsewhere.
