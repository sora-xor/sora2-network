# SORA Nexus retained-history replay

The primary bootstrap compatibility witness executes the deployed Wasm and the exact candidate Wasm against identical complete historical parent states. It requires both executions to produce the retained header's state root. The tool never substitutes native execution or changes the stored `:code` to select its candidate. These are compatibility witnesses; they do not certify recovery execution, consensus signatures, operator readiness or production release readiness.

## Selecting real boundaries

`discover_history.mjs` probes the approved archive for the latest retained changes in `Session.CurrentIndex`, `Babe.EpochIndex` and runtime specification. It searches the observed counters rather than choosing an arbitrary recent block window. Its observations select test inputs only: the Rust replay independently checks that the claimed boundaries actually occur during execution.

```sh
node misc/incident_recovery/discover_history.mjs \
  --dependencies runtime-rehearsal-sdk2609-2026-10-25/validation/package.json \
  --output artifacts/recovery-28191817/history/discovered-range-plan.json
```

The initial observed plan selects checkpoint block 28,191,817, the session/epoch transition at 28,191,268 and its following block, and the spec-133 to spec-134 transition at 28,073,392 and its following block. The first upgrade block executes deployed spec 133 on both branches; its first child exercises the candidate's preserved spec-134 migration path. These heights are recorded observations, not hardcoded discovery assumptions.

## Capturing and executing

All RPC methods are explicitly read-only. Only `wss://mof2.sora.org` and explicit loopback endpoints are admitted. Capture is resumable, preserves completed files, records page checksums and supports child tries. It never claims a state root is verified merely because pagination completed.

```sh
node misc/incident_recovery/capture_history.mjs \
  --plan artifacts/recovery-28191817/history/discovered-range-plan.json \
  --candidate /absolute/path/to/immutable-bootstrap-134.wasm \
  --output /absolute/new/evidence-directory

incident-recovery replay-history \
  --manifest /absolute/new/evidence-directory/replay-manifest.json
```

Parent captures and ancestry can also run separately with `--parent NUMBER` or `--ancestry-start NUMBER`, each using `--output DIR`. Do not run two writers for the same capture or ancestry directory. Parent captures refuse overlapping live writer locks. Ancestry uses at most 128 lightweight reads per JSON-RPC batch and waits for each batch before requesting another.

The manifest binds the candidate, every block body and the consecutive retained header chain with SHA-256. The ancestry ends at the immutable checkpoint hash; the verifier recomputes every SCALE header hash and parent link. For each range it reconstructs the complete parent trie and checks its authenticated header root, validates the block's extrinsics root, strips exactly one trailing BABE seal, and invokes `Core_execute_block` with Wasm execution. Its resulting root must equal the retained header root on both branches. Unsupported historical substitutes are rejected rather than silently bypassed.

Manifest fields are `schema`, `checkpointHash`, `candidate`, `ancestry` and `ranges`. Artifact references have `file` and `sha256`; paths must remain inside the evidence directory. Each range has `name`, `rationale`, `boundaries`, `parentCapture` and ordered `blocks`. Valid boundary labels are `session`, `epoch`, `upgrade` and `checkpoint`.

The witness reports `representativeBoundaryCoverage` only when all four boundaries occurred in execution. A checkpoint-only witness deliberately leaves that field false. It always reports `consensusSealsVerified: false` and `productionReady: false`: full/warp synchronization, forced GRANDPA transition, actual recovery chunks and final activation have separate gates.

An unbound diagnostic bootstrap can exercise all preserved blocks but deliberately refuses the first recovery child. Any later bound build has different bytes and needs its own exact-artifact replay; passing an unbound build cannot admit the bound release.

## Exact maintenance execution

`incident-recovery rehearse-maintenance --manifest manifest.json --output NEW_DIR`
executes the exact bound bootstrap, hold, repair and final Wasm artifacts against
the independently verified checkpoint snapshot. `NEW_DIR` must not exist. The
command writes only its own block files and immutable trie snapshot objects;
it never opens a node database or submits transactions.

The `sora-nexus-maintenance-replay-v1` manifest supplies `checkpointHash`, a
relative `checkpointCapture` directory, `policy`, `bootstrap`, `hold`,
`finalRuntime`, optional `repairs` artifact references, `previousGrandpaSet`,
the ordered `freshGrandpa` public key/weight pairs, and `steps`. The field name
is retained for schema compatibility; current retained-key releases place the
unchanged checkpoint authority list there, not newly generated keys. Each artifact
reference uses `file` and `sha256`. Each step supplies `slot`, `timestamp`,
`authorityIndex`, `recovery` (an artifact containing `encode_remark` bytes, or
`null` after activation), `expectedSpec`, and `expectedPhase` using the runtime
phase names (`Applying`, `Auditing`, `Ready`, `Hold`, `Complete`, `Failed`).
Optional `rejectedRecovery` references exercise envelopes that must fail without
changing state. All paths stay inside the manifest directory.

Every artifact's identity is checked by executing `Core_version`. Each block is
authored through the runtime APIs, independently imported from the prior disk
snapshot through `Core_execute_block`, and required to produce identical state
roots. The first snapshot writes the complete trie. Each following block writes
only changed raw trie nodes and removals, plus a SHA-bound manifest chaining its
parent snapshot. Every import and restart reconstructs a fresh externalities
instance from the persisted base and delta chain. A hash of the complete sorted
raw-node inventory proves that reconstruction exactly matches the SDK snapshot;
the state root must also match. Thus every block retains restart coverage without
rewriting hundreds of full snapshots. It rejects ordinary
remarks and repeated recovery envelopes during maintenance, verifies frozen
session/era state, checks first-child retained GRANDPA set installation, confines
non-incoming financial writes to committed edits, and requires persistent bridge
halt and a unique lineage digest. The sequence must install the exact pinned
138 artifact and execute at least one ordinary runtime block. The report records
whether hold, forward repair, liveness grace and fee-clock shifting were actually
observed; omitted paths do not become passing gates.

To establish the first recovery child's hash before signing an escape, use the
separate `sora-nexus-maintenance-prefix-v1` manifest schema with the same command.
It accepts exactly one first child containing either `Tick` or the committed
`Apply` chunk at index zero. It rejects repairs, signing, native recorders,
fixture overrides, staging and final-state evidence requests. Checkpoint,
exact-Wasm execution, independent import, persisted restart and frozen-session
checks still run. The terminal must retain deployed code at spec 134, the
original policy and operation counter zero, with cursor zero or one as specified
by that first action. Its report uses
`sora-nexus-maintenance-prefix-witness-v1`; it is not an activation or release
certificate. The normal replay schema still requires automatic final activation.

The generated blocks have no validator seals. This exercise does not establish
full/warp client synchronization, network finality, operator control, signing-stop
evidence, or production release readiness. Authenticated incoming applications
retain their separate financial/nonce/acknowledgement tests. Public fixture
bindings and deterministic fixture signing keys remain explicitly non-production;
the package verifier rejects fixture provenance.

## Structural mixed-work assembly

Build the offline utility with `--features runtime-work-types` to enable
`incident-recovery compile-mixed-work --manifest FILE`. This opt-in feature
uses the runtime's actual incoming/outgoing SCALE types and commitment methods;
it does not reproduce those encodings in a second codec. In particular, HASHI
templates bind the native approval call/extra and signer while excluding the
future signature bytes exactly as the runtime does.

The bounded JSON input has schema `sora-nexus-mixed-work-input-v1` and fields
`policyTemplateScale`, `workEnvelopes`, `auditEnvelopes`, and `terminalEnvelope`.
All byte strings are canonical lowercase `0x` hexadecimal. Work envelopes are
complete `encode_remark` bytes containing only `Apply`, `Incoming`, or `Outgoing`;
audit envelopes contain only `Audit`, and the final field contains `TerminalAudit`.
Their indices must already be consecutive from zero, independently for work and
audits. Caller-reviewed ordering is preserved. The policy's work/audit hash lists
must be empty or exactly match the supplied work; its terminal hash must be zero
or exactly match the supplied terminal audit. Stale commitments are rejected.

The output is `sora-nexus-structural-mixed-work-v1`, with the assembled policy and
unchanged envelopes. Duplicate work is rejected after normalizing its index,
including HASHI templates differing only in future signature bytes. Repeated
audit storage locations, malformed encodings, oversized input and non-work actions
are also rejected. This checks structure only: signatures, message authentication,
state preimages, complete inventories, economic conservation and artifact execution
remain unverified. All corresponding output flags remain false. Passing this
compiler cannot satisfy checkpoint admission, exact-Wasm rehearsal or release gates.
