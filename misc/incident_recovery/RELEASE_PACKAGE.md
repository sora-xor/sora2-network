# SORA Nexus recovery release packaging

`release_package.py` prepares a local, content-addressed candidate bundle.
Use Python 3.14 or newer for the standard-library Zstandard decoder required by
compressed runtime Wasm. The tests and development run use Python 3.15. It does
not publish artifacts, alter a service, restart a node, or authorize activation.
The package always states `productionReady: false`: independent exact-artifact
execution, full/warp sync and recovery-scenario verification remain required.
Historical replay and recovery Wasm identity have the narrow optional
fresh-execution hooks described below. There is no force, skip, or promotion flag.

`finalRuntimeActivationPostState` is a separate mandatory gate. Its fresh native
driver replays every committed maintenance operation under the real release
policy, then executes the exact final Wasm on that resulting state. Before the
first final-runtime initialization it records the completed maintenance parent
hash, state root and persisted snapshot, the pre-upgrade runtime marker and
staking version 16. After initialization it requires runtime 138, staking version
18, the expected legacy cutoff/first-execution eras and incentive start marker.
At least two ordinary final-runtime blocks must then pass independent Wasm import
and persisted-state restoration. A migration panic, mismatched policy/artifact,
missing observer, one-block sample or mere `set_code` installation cannot clear
this gate. The successful observation is registered only after the entire fresh
process and its artifacts pass the final input/output guard.

This executes the production `on_runtime_upgrade` through `Core_initialize_block`
and `Core_execute_block`. It explicitly does not claim that the separately
compiled `try-runtime` feature checks ran against that exact production Wasm.
Those supporting checks and later custom staking/election/reward scenarios
remain separate requirements. No corrective spec-134 substitute is permitted
after activation; passing this gate is required before the restart is authorized.

A candidate JSON uses this exact shape (replace the illustrative paths and
hashes with actual public release inputs):

```json
{
  "format": "sora-nexus-recovery-release-candidate-v1",
  "checkpoint": {
    "number": 28191817,
    "hash": "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0"
  },
  "nodeVersion": "4.8.13",
  "artifacts": [
    {
      "role": "bootstrapWasm",
      "path": "runtime/bootstrap-134.wasm",
      "sha256": "64 lowercase hexadecimal digits, without 0x",
      "blake2_256": "64 lowercase hexadecimal digits, without 0x"
    }
  ],
  "evidence": [
    {"gate": "retainedHistoryStateRootReplay", "artifacts": ["historicalReplayEvidence"]}
  ]
}
```

The recovery chain extension uses schema version 2 and carries `policyScale` as
hexadecimal bytes in addition to `policyCommitment`. Node admission decodes the
bounded policy, checks its hash and evidence commitment, and requires its
`authorizationDomain.chainSpecId` to match the recovery network ID. The packager
requires identical policy bytes in the chain extension and the bound policy
artifact. Policy format 2 also commits the release nonce; recovery signatures from
another network or nonce are invalid. All repair policies retain this domain.

Every evidence reference must name an artifact in the inventory. Evidence entries
bind raw files; they cannot assert a pass result. Top-level readiness fields and
per-gate pass booleans are rejected. Additional artifact roles are allowed for
raw histories, rehearsal inputs/results, metadata witnesses and build records.
Required roles are listed in `REQUIRED` in the tool. Missing roles are reported as
release blockers, so partial local bundles are useful without being mistaken for
a release. All paths are relative to the candidate directory; parent traversal,
symlinks and known credential-material paths are rejected. Supply only public
inputs. Do not include private operator keys or service credentials.

`attackerCoverageEvidence` binds the compiler's complete native typed identity
watch, including the fixed AccountId32/H160 and additional approved identities.
It does not establish beneficial ownership or replace network-specific attacker
registries, external claims, LP/debt settlement or burn-history evidence.
`bridgeSigningReview` supplies the exact complete checkpoint Approvals,
PendingApprovals and both outbound LatestCommitment prefixes consumed by
`compose-storage-work --bridge-signing`. Known signing hashes receive permanent
local tombstones while every original signature and commitment remains intact.
This does not invalidate external authorizations or assert historical batch
membership for hashes whose payloads are unavailable. Neither artifact clears
a release gate merely by being present.

`nativeHashiReview` binds ordered commitments for the complete native HASHI
request, status, approval, approver, submission-height, queue and account-index
namespaces. The native compiler verifies these commitments against the checkpoint
and permanently stops every captured outgoing request, including those absent
from BridgeProxy's sender graph. Original signing evidence remains unchanged;
there is no inferred refund or external invalidation. Native request identities
come from the map key and stored wrapper hash, never a reconstructed hash of
financial fields that may have changed historically.

`incomingDeliveryInventory`, `incomingReconciliation` and `recoveryWorkIndex`
provide the [incoming obligation coverage inputs](INCOMING_COVERAGE.md). The
packager binds every bounded work/audit shard and the terminal envelope. Its fixed
native verifier independently enumerates checkpoint HASHI transfers/loads and
requires a disposition for every unconsumed local record, declared external
delivery and actual typed incoming work message. Admission cannot be replaced by
quarantine; retained liabilities require exact hashes and final audit witnesses.
The resulting `incomingPendingCoverage` gate does not clear
`bridgeReconciliation`, external finality/authentication or overall readiness.

When incoming work was consumed before a council-authorized forward repair, add
both `recoveryParentReplay` and `incomingActivePolicyScale`. The first is the
complete `sora-nexus-supplied-council-maintenance-replay-v1` manifest; the second
is the exact active policy used by the repaired work/audit index. Keep
`policyScale` as the original policy: the chain identity, immutable134
substitute, hold135 and final138 bindings continue to use it. The paired roles
cannot be supplied independently. A terminal JSON assertion cannot establish
that the active policy was installed.

The fixed incoming verifier then runs the supplied recovery manifest in the same
process as coverage checking. Only successful exact-Wasm execution, independent
block import and persisted-state restoration can produce its opaque in-memory
certificate for an earlier incoming operation and its later typed audit. The
packager additionally joins the serialized checkpoint, manifest, transcript,
original/active policies, consuming/audit blocks and native comparisons. It
cannot import a certificate from a user-supplied JSON success report.

Include every referenced runtime, envelope, raw recovered-council signature,
native recorder executable and engine binding under the candidate directory.
The checkpoint capture must be the same bound capture. Native recorder binaries
must be independent read-only executable copies with mode555. Public test
councils, public fixture manifests/fields, fixture native bindings and fixture
repair provenance are rejected. Real ownership proofs, recovered-council
inventory, fresh operator keys/signing-stop evidence, per-network attacker
registries and authenticated external reconciliation remain required release
inputs. This option does not replace them or approve their provenance.

Each referenced repair136 or137 additionally needs four fixed roles (replace136
with137 for that version):

- `repair136PolicyScale`: the exact policy compiled into that repair.
- `repair136Binding`: the existing repair-export binding, including original and
  active policy bytes, operator/consensus parameters and final138 Wasm.
- `repair136Provenance`: a bound non-fixture preserved-source export whose
  binding hashes match those exact inputs.
- `repair136BuildReceipt`: the successful build receipt naming that provenance
  and exact repair artifact, with `cargo.stdout.log` and `cargo.stderr.log`
  beside it and their matching digests in the receipt.

These records bind declared build provenance and reject mismatched or fixture
artifacts. They do not independently attest compilation; the source/build and
exact-artifact gates still apply. Intermediate repair policies keep their own
roles rather than replacing the original policy or final138 provenance.

The packager owns a fresh scratch directory and supplies its `--recovery-output`
path; the manifest cannot choose a command or output destination. It copies the
explicitly pinned verifier, records its actual PID/exit/logs and all pre/post
input hashes, and retains failed attempts. Successful replay, native comparison,
snapshot and process files are included in content-addressed package objects
under `generated-incoming-replay/` reconstruction paths. No prior output is
replaced. Reconstructing those objects checks integrity, not runtime execution.

The `incomingPendingCoverage` gate remains local coverage only. Even with an
accepted prior-policy certificate, external finality, consensus seals, operator
approval and overall `productionReady` remain false; bridge reconciliation and
all other release gates are independent.

```sh
python3 misc/incident_recovery/release_package.py \
  --candidate /absolute/path/to/public-inputs/candidate.json \
  --output /absolute/path/to/new-release-bundle
```

The verifier independently recomputes both hashes for every file. It validates
native SCALE policy and operator inventories, including mapping uniqueness,
attacker exclusion, council membership, fresh/retired key disjointness and exact
CSV-to-policy pairs (SS58 and canonical hexadecimal addresses are supported).
It checks the recovery network identity, authority weights, signing-stop slot
boundary, distinct runtime versions and code hashes, duplicate substitute spec
versions, and the exact bootstrap bytes at checkpoint height. Bootstrap and hold
build bindings must pin the same policy/operators and exact destination Wasm.
Their provenance must name the pinned deployed source tree and stable2603-3;
the recorded binding hashes must match the supplied files. A `bound` field by
itself does not establish those relationships or prove the build output.

Runtime versions extracted from Wasm custom sections are labeled as embedded
identity claims. They do not substitute for executing `Core_version`, checking
recovery metadata by actual executing-code identity, or proving the code was
built from the bound source. The node version is likewise an expected release
version until independent executable-artifact verification completes. A source
report that says `allChecksPass` does not clear any gate.

For a subsequent candidate, provide the prior trusted outer manifest:

```sh
python3 misc/incident_recovery/release_package.py \
  --candidate /absolute/path/to/public-inputs/candidate.json \
  --previous-manifest /absolute/path/to/prior-release/release-manifest.json \
  --output /absolute/path/to/new-release-bundle
```

The prior manifest's checkpoint and bootstrap code hash must agree. A different
spec-134 bootstrap is rejected. The prior manifest SHA-256 is recorded. This
continuity check assumes the operator supplies the previously approved manifest;
it does not invent a signing authority or authenticate an arbitrary prior file.

The optional native verifier must be an explicitly supplied, locally approved
`incident-recovery` executable with a separately pinned SHA-256:

```sh
python3 misc/incident_recovery/release_package.py \
  --candidate /absolute/path/to/public-inputs/candidate.json \
  --evidence-verifier /absolute/path/to/incident-recovery \
  --evidence-verifier-sha256 ACTUAL_APPROVED_BINARY_SHA256 \
  --output /absolute/path/to/new-release-bundle
```

Only fixed local `verify-capture`, `verify-metadata`, `verify-ownership` and
`verify-incoming-coverage`
commands are invoked; candidate files cannot supply executable commands. The
checkpoint capture manifest must be named `capture-complete.json`. Every file
and page referenced by that manifest is hashed and automatically included in the
outer bundle. Fresh verifier results are bound to the supplied capture manifest
and mapping. The required `semanticPlan` is the exact plan consumed by storage
composition; `derivedAccountInventory` is its exact `derivedAccounts` JSON array
(use `[]` when there are no derived entries). Both artifacts are required even
for a direct-key-only release. They cannot contain alternative executable
commands or approval booleans in place of native control evidence.

For ownership verification, the packager always invokes the complete path:

```sh
incident-recovery verify-ownership \
  --csv /absolute/path/to/mapping.csv \
  --proofs /absolute/path/to/ownership-proofs.json \
  --capture /absolute/path/to/checkpoint \
  --semantic /absolute/path/to/semantic-plan.json \
  --derived-inventory /absolute/path/to/derived-accounts.json
```

The native verifier reconstructs the checkpoint and executes its own Wasm to
authenticate the metadata, then checks every CSV pair through the same bounded
direct/derived control graph used by composition. It compares the independent
derived inventory with the semantic plan. Fresh receipts bind the raw CSV,
proofs, plan, derived inventory and capture manifest hashes to the originally
admitted candidate; changing a file after candidate validation cannot clear a
gate. The direct-only CLI remains available without the three graph flags, but
the release packager never substitutes it for missing semantic/checkpoint inputs.
These checks clear only ownership/control of the submitted graph. Discovering
every derived account, unused destinations, historical control changes and full
storage semantics remain independent gates.

Checkpoint metadata execution does not clear recovery-artifact
execution. Header, extrinsics and state-root verification does not clear
checkpoint finality or retained-history gates. Original source reports never
replace these fresh executions.

The output contains:

- `objects/<sha256>`: read-only copies of all inputs, deduplicated by content;
  executable inputs retain executable permission.
- `release-manifest.json`: artifact hashes, original relative paths, policy and
  runtime identities, verifier receipts, missing inputs and outstanding gates.
- `SHA256SUMS`: checksums of every content object and the outer manifest.
- `PACKAGE-COMPLETE`: written last, containing the outer manifest SHA-256.
- `README.txt`: an explicit warning that this remains a local candidate.

The original relative paths in `sourcePath` allow an auditor to reconstruct the
input layout, including checkpoint pages and build-binding references, from the
content objects. No existing output directory is replaced. An interrupted move
has no `PACKAGE-COMPLETE` marker and is not a complete package. Static package
checks, fresh local proof checks and unresolved release gates remain separate
in the manifest. Tests use synthetic Wasm/headers only; their fixtures are never
production evidence.

Run the offline tests without Cargo:

```sh
python3 -m unittest discover -s misc/incident_recovery -p 'test_release_package.py'
python3 -m unittest discover -s misc/incident_recovery -p 'test_hashi_codec.py'
```

The `operatorsScale` artifact encodes a SCALE vector of operator records in this
order: checkpoint `AccountId32`, `Vec<GrandpaId>` of retired active/queued/NextKeys
keys (sorted, one to three), fresh `GrandpaId`, `Vec<BabeId>` of independently
approved retained active/queued/NextKeys keys (sorted, one to three), and
`Vec<ImOnlineId>` with the corresponding approval union (sorted, one to three).
No key may belong to two operators within its signing domain. The bootstrap
checks exact observed unions before writes; the package verifier rejects the
older GRANDPA-only record layout. These are public keys, never keystore data.

The required `stakingRoleReview` artifact is the exact review consumed by the
native role compiler. Its `operatorsHash` must equal the Blake2-256 digest of
`operatorsScale`, which the bootstrap and hold bindings also pin. A different
operator list cannot authorize retention of a staking role. This equality does
not establish operator approval: the public approval evidence and native
role/nomination proof checks remain release gates.

The required `burnHistoryManifest` role binds the canonical raw Burn-event proof
stream. A finite diagnostic capture can reference its initial header, compressed
pages and content-addressed proof nodes directly. The packager traverses that
reference graph and checks every file hash. Missing or mutated proof objects
reject packaging; a report saying “verified” cannot substitute for those bytes.

The complete campaign history exceeds the direct artifact-count limit. Package
it as deterministic archive shards, retaining the raw manifest and every
referenced page/proof node:

```sh
python3 misc/incident_recovery/archive_burn_evidence.py \
  --manifest /absolute/path/to/burn-history-manifest.json \
  --destination /absolute/new/burn-evidence-archive
```

Shards default to 256 MiB. SQLite bounds the in-memory deduplication work; each
canonical tar shard contains an internal SHA-256 inventory. The archive verifier
checks the entire reference graph, including missing/extra members and all
internal digests. Supply its `burn-evidence-archive.json` as the
`burnHistoryManifest` artifact. The packager verifies the archive and includes all
its shards without relaxing the outer artifact limit.

Archive integrity is not a trie-proof, allocation or finality check. Reconstruct
the exact raw input files into a new directory and run the native verifier:

```sh
python3 misc/incident_recovery/archive_burn_evidence.py \
  --restore /absolute/path/to/burn-evidence-archive.json \
  --destination /absolute/new/restored-burn-evidence
incident-recovery verify-burn-history \
  --manifest /absolute/new/restored-burn-evidence/burn-history-manifest.json \
  --output /absolute/new/verified-burn-evidence
```

The restorer never overwrites a directory, does not extract symlinks or special
files, and rechecks each shard as it is copied. Neither archiving, restoring nor
static packaging clears a production verification gate. Public fixture runtime
provenance is explicitly rejected by the release packager.

The Burn gate now has a separate fixed execution path. Build the companion
verifier locally from the reviewed `polkaswap-indexer` source, without installing
packages or contacting a service:

```sh
node scripts/build-burn-release-verifier.mjs --output /absolute/new/burn-verifier
```

The create-new output contains one bundled `burn-recovery-verifier.mjs` and its
transitive source/lock provenance. Add release artifacts with roles
`burnRecoveryVerifier`, `burnRecoveryVerifierProvenance`, `burnNodeBinary`,
`burnNativeVerifier` and `burnAllocationPolicy`. The two binary roles are exact
copies of the externally reviewed Node and native recovery verifier executables.
The allocation policy is the preactivation policy described in `BURN_HISTORY.md`;
post-fork anchors and an outer release hash are deliberately excluded from it.

Alongside the existing native verifier flags, supply all four explicit pins and
output options to the packager:

```sh
python3 misc/incident_recovery/release_package.py \
  --candidate /absolute/path/to/candidate.json --output /absolute/new/release \
  --evidence-verifier /absolute/path/to/incident-recovery \
  --evidence-verifier-sha256 REVIEWED_NATIVE_SHA256 \
  --burn-node /absolute/path/to/node --burn-node-sha256 REVIEWED_NODE_SHA256 \
  --burn-verifier-sha256 REVIEWED_BUNDLED_VERIFIER_SHA256 \
  --burn-verification-output /absolute/new/burn-verification
```

This invokes only the fixed bundled workflow: restore archived source evidence
when applicable, execute the native full-history proof verifier afresh, decode
with the shared live/backfill event extractor, and run the exact allocation
compiler. It compares the resulting ledger byte-for-byte with `burnEntitlements`.
The allocation mapping must equal both the CSV and compiled runtime policy; its
SORA attacker set must equal the AccountId32 entries of the bound native typed
attacker registry. Missing history, changed input bytes, incomplete ranges or
supplied success reports cannot clear this gate. Fresh receipts and logs remain
in the new verification directory. The default execution timeout is one day;
`--burn-timeout` permits an explicit limit up to seven days for a full local
history verification. A timeout never authorizes partial evidence.

The outer bundle automatically includes fresh verifier stdout/stderr, completion
receipts, the canonical receipts, the rederived ledger, and every native verified
page and executing-metadata object. The large native output graph is retained in
bounded deterministic archive shards with its own checked reference graph; it
does not consume one outer artifact role per historical page. Generated roles
begin with `burnVerified`, and reconstructed paths begin with
`generated-burn-verification/`; candidate artifacts cannot occupy these reserved
names. Changed generated evidence fails the final copy/hash checks. To inspect
the reconstructed native output, restore its archive into a new directory:

```sh
python3 misc/incident_recovery/archive_verified_burn_evidence.py \
  --restore /absolute/path/to/verified-burn-evidence-archive.json \
  --destination /absolute/new/verified-burn-history
```

This derived archive complements the separately retained raw proof archive.
Restoring it checks integrity; proof verification still requires the raw history
and the pinned verifier.

Only `burnEntitlementsAndClaimHolds` is cleared by this execution. It proves the
ledger's replacement ownership, unchanged attacker contribution to the TS curve,
extinguished attacker entitlement and held replacement-recipient allocations.
It does not establish consensus finality, beneficial attacker ownership,
claim-service deployment or destination-chain settlement. Other gates remain
independent and the package still reports `productionReady=false`.

## Fresh retained-history replay

Add the optional `historicalReplayManifest` artifact role to run the existing
`replay-history` command when supplying `--evidence-verifier` and its explicit
SHA-256 pin. Without that manifest or executable, the history gate remains
unverified. An archived success report or evidence entry cannot clear it.

The manifest must be `sora-nexus-historical-replay-v1`, name the exact packaged
`bootstrapWasm`, and include representative session, epoch, upgrade and checkpoint
ranges. The bootstrap must have its complete existing release policy, operator,
binding, provenance and chain-spec inputs. Public fixture provenance is rejected.
Every ancestry header, block, parent capture manifest, top/child trie page and
capture artifact is hash-bound, local to the candidate directory and retained in
the release. The header chain must end at the pinned checkpoint hash and root.

The hook makes a fresh private scratch directory separate from incoming replay,
copies the bound input graph read-only, copies the pinned native verifier as an
independent mode555 executable, and runs only `replay-history --manifest ...`.
Historical replay has a three-hour (10,800-second) execution limit to allow
for a busy machine. It records the actual PID, command, exit status, stdout/stderr,
source/input pins and post-execution guard. Failed runs remain in their scratch directory; no
failed report is packaged as success. Successful outputs and executable/source
copies are content-addressed in the bundle, with reconstruction paths.

Successful native Wasm execution must match each anchored block and parent root,
exercise the bootstrap in every range, and observe all four boundary classes.
Only `retainedHistoryStateRootReplay` is then verified. Build provenance
authenticity, consensus seals, finality, full/warp synchronization, operator
approval and all other gates are unchanged. Overall `productionReady` remains
false. The native verifier digest is an explicitly supplied trusted executable
pin; this hook does not establish who built or approved that executable.

For recovery Wasm identity, supply both optional roles
`artifactActivationReplay` and `artifactHoldReplay`, together with the explicitly
pinned native verifier. These are existing native maintenance manifests, with
their complete local checkpoint, runtime, policy and envelope reference graph.
They must use the exact package checkpoint capture, original policy,
bootstrap134, hold135 and final138 bytes, previous GRANDPA set, fresh authorities
and first recovery slot from the chain specification. Fixture provenance,
fixture fields, known public test signing keys across roles, repair versions and
alternate policies are rejected by this narrow hook.

The activation manifest uses `sora-nexus-maintenance-replay-v1`. It must finish
automatic activation and include at least one subsequent block executed by the
exact final138 Wasm. Incoming/outgoing work requires the existing original-policy
native recorder, its independent mode555 executable and exact nonfixture engine
binding. Its auxiliary native comparisons do not replace Wasm authoring or
import. The report's `nativeExecution` flag records those auxiliary comparisons;
it is not evidence that block execution bypassed Wasm.

The hold manifest uses `sora-nexus-supplied-council-maintenance-replay-v1`. Its
first recovery child must carry the actual recovered-council hold135 escape,
followed by at least one block executed by hold135. The terminal remains Hold,
with unchanged policy, zero work/audit cursors and operation counter1. Obtain
the deterministic expected terminal root through an offline diagnostic first;
the package hook then executes a fresh replay against that committed root.
Council signatures are supplied public release inputs. The hook neither
generates signatures nor establishes approval of the council inventory.

Each scenario runs the fixed `rehearse-maintenance --manifest ... --output ...`
command in a newly owned directory, separately from incoming and historical
replays, with a 24-hour limit to accommodate busy machines and complete state
restoration. Inputs, helper sources, the independent verifier copy, actual
process receipts, blocks, transcripts and the complete snapshot graph remain
hash-bound through final output registration. Failed attempts are retained.
Changing an accepted file aborts packaging; archived success reports cannot
clear this gate. Successful output uses `generated-artifact-identity/`
reconstruction paths.

Only `executingArtifactIdentity` is verified by this hook. The trusted fresh
native CLI executes `Core_version`, checks transaction versions131/132/134,
authors and independently imports every block through Wasm, and reconstructs
persisted state. The packager joins each block's actual executing code hash,
ancestry, policy marker, timestamp, recovery envelope and restored-state witness
to the package. Merely installing a runtime or reading its custom section is
insufficient. This gate covers the three recovery Wasm artifacts; node identity,
source/build authenticity, consensus, finality, full/warp sync, operator approval,
forward repair and all other gates remain separate. `productionReady` stays false.

The optional `forwardRepairReplay` role enables a separate fresh forward-repair
verification. Give it the same file path as `recoveryParentReplay`; both roles
must identify the exact manifest already bound by incoming-coverage admission.
The identity pair above and all incoming-coverage inputs remain required. Each
repair136/137 needs the existing policy, binding, preserved-source provenance,
build receipt and logs. The hook admits only the forward branches 135→136,
135→137 or 135→136→137 and requires a real block executed by every listed runtime,
followed by automatic runtime 138 activation and an ordinary runtime 138 block. It does not demand
an unused intermediate version.

This hook uses a fresh fixed `verify-incoming-coverage --recovery-replay ...`
invocation in its own copied input graph and output directory. That existing
native command alone provides the opaque, in-process historical incoming-audit
certificate needed here. It must preserve prior authenticated incoming operations
and their audits through the complete final active policy, with no pending audit
left behind. A separately serialized certificate cannot enter the native checker.
The ordinary identity pair, the incoming gate and this repair gate are separate
executions; success is never copied between their reports.

Raw council authorization bytes and signing messages must match the checkpoint,
original policy, observed phase/cursor/counter, executing and installed code,
destination version and exact signed revised policy. Each old authorization is
tested for replay rejection by the native rehearsal. Native-v2 dynamic work must
match its accepted escape prefix, active policy, exact engine and handover
lifecycle. The handover authorization exists only for the first repaired child;
later native comparisons require its absence. All block, transcript, snapshot,
native process and output bindings remain guarded through final registration.

The current admission corridor requires at least one historical incoming audit
and an actual dynamic operation compared by a repaired native engine. A release
with no incoming work or no historical audit is unsupported by this hook; do not
invent an empty inventory or weaken coverage to bypass it. The fixed replay has
a 24-hour bound and retains failed attempts. Successful objects use
`generated-forward-repair/` reconstruction paths.

Only `automaticActivationEscapeAndForwardRepair` changes after this check.
The identity gate must already have passed with the same pinned verifier and is
left unchanged. Native source/build authenticity, council inventory approval,
external finality, economic policy approval, full/warp synchronization, actual
process-crash recovery and maintenance epoch/liveness/custom-staking coverage
remain separate. `productionReady` remains false.
