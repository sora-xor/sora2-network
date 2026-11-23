# SORA Nexus supporting checks and fresh artifact rebuild

This optional release-package hook executes the fixed mainnet, try-runtime and
extended Clippy modes, the runtime library tests, the recovery-tool tests with
`bridge-fixture`, and strict all-target recovery-tool Clippy. It uses the reviewed
source tree, locked offline Cargo commands and an explicitly supplied persistent
target. It never cleans a target, chooses a job count, installs a toolchain or
publishes an artifact.

The hook runs after the package's independent evidence hooks. It retains actual
process launch/start/exit receipts, logs, the complete checked source, helper
bytes, compiler-launcher identities and the current package's exact artifact and
gate identities. A new file or changed source, input, helper, prior receipt or
output aborts registration. Historical reports and caller-supplied success flags
are not accepted as successful checks.

The hook launches the fixed `production_build.py build` corridor afresh. Its
input manifest is data only; it cannot supply commands. The returned manifest
digest comes from the builder's in-process result and joins the complete output
graph and freshly rebuilt 134/135/138 bytes to the exact package artifacts. The
rebuilt policy must equal the package's **original** policy. The source inventory
must equal both the reviewed production inventory and the corresponding subset
of the supporting inventory. This is a real local reproducibility comparison,
not admission of an archived build receipt.

If the release includes repairs, the hook derives their finite ordered set from
the already freshly verified forward-replay context. It supports 136, 137, or
136 then 137. Every declared repair must have the exact active-policy, original
lineage, operator/consensus bindings and Wasm bytes observed in that replay. Each
engine receives a new source export from the fixed bootstrap preparer, followed
by the existing `production_native.py build-engine-wasm` corridor in the same
persistent legacy target. Export stdout pins the exact in-process provenance;
engine stdout pins its completion result, compiled source, original/active
policies, actual Cargo launch/PID/log receipts and artifact bytes. Missing, extra,
or unreproduced repair roles cannot clear the aggregate gate.

Only when that fresh build join succeeds **and every other required release gate
is independently verified in the same package invocation** may
`fullClippyRuntimeAndProductionRehearsals` become verified. Missing economic,
maintenance, crash, consensus, external or operator evidence stays missing.
Overall `productionReady` remains false; this module does not implement final
release authorization, publication or restart.

Final runtime 138 deliberately retains the **original** compiled policy. Its
replacement lookup compares that commitment with `POLICY_STORAGE_KEY`; a repair
updates only `ACTIVE_POLICY_STORAGE_KEY`. Substituting the last active policy
into final138 would break the original account-lookup lineage. Repair136/137
therefore compile both original and active policies, while 134/135/138 and the
shared final Wasm bytes remain unchanged by repair reproduction. The control
prefix (checkpoint, substitutions and council) must be identical across them.

This does not authorize a new repair or accept signatures by inspection. Fresh
forward execution and every other required release gate remain prerequisites.
The existing incoming receipt admission receives adjacent `cargo.stdout.log`
and `cargo.stderr.log` copies bound to the actual compiler's log pins; the nested
actual process receipts remain retained as well.

## Inputs

Prepare the normal nonfixture production build input graph, including signed
mapping/control evidence, operators and consensus parameters, as described in
`PRODUCTION_BUILD.md`. Public fixture locations, markers and known public signing
keys are rejected by the existing production input loader. This workflow does
not request or use private keys.

Generate two source inventories after all reviewed source changes are applied:

```sh
python3 -B misc/incident_recovery/production_build.py inventory \
  --output /approved/recovery-inputs/production-source.json
python3 -B misc/incident_recovery/supporting_checks_release.py \
  --inventory-output /approved/recovery-inputs/supporting-source.json
```

The second command prints the fixed supporting source binding. Save and review
that value before execution. The supporting inventory additionally covers the
Clippy scripts and LLVM wrapper. The build helper located outside those source
paths is independently pinned and retained.

Use this manifest shape; each `sha256` is the exact digest of the referenced
local file and every path is relative to the manifest's directory:

```json
{
  "schema": "sora-nexus-supporting-check-input-v1",
  "sourceInventory": {"file": "production-source.json", "sha256": "..."},
  "fullSourceInventory": {"file": "supporting-source.json", "sha256": "..."},
  "buildInputs": {"file": "production-inputs.json", "sha256": "..."},
  "legacyLock": {"file": "resolved-legacy-Cargo.lock", "sha256": "..."},
  "finalLock": {"file": "normalized-final-Cargo.lock", "sha256": "..."}
}
```

`finalLock` may be null only when the current final lock already builds with
`--locked`. Lock normalization uses the existing dependency-neutral production
builder validation; this hook never silently rewrites the working-tree lock.

Add all six options to the normal `release_package.py` invocation, together with
its explicitly pinned native evidence verifier and required production evidence:

```text
--supporting-check-manifest /approved/recovery-inputs/supporting.json
--supporting-check-source-sha256 <reviewed fixed source binding>
--supporting-check-output /approved/recovery-runs/supporting-attempt-1
--supporting-check-target /approved/recovery-targets/checks
--supporting-final-target /approved/recovery-targets/final
--supporting-legacy-target /approved/recovery-targets/legacy
```

The output must be new and outside the working source. All three target lanes
must already exist and be distinct; the final and legacy lanes must be outside
the source. They use the existing target lock and remain warm across attempts.
The lanes must not be ancestors or descendants of one another. Do not run a
competing Cargo process in those lanes. Only the reviewed root `.cargo`
configuration is admitted; ambient home or ancestor Cargo configuration causes
rejection without reading its contents. A separately prepared Cargo cache may
be selected through `CARGO_HOME`, provided it has no unreviewed config. No
credentials or Cargo cache files are copied into the release evidence.

Each check allows 24 hours. Each internal Wasm build already allows 24 hours;
the parent production-build process allows 96 hours for the original triple.
Each repair export allows 24 hours and each repair-engine parent 48 hours, with
the actual inner Wasm build still bounded at 24 hours. Output bounds and owned process-group cleanup remain
active. A failed attempt retains its evidence and cannot promote a gate. Use a
new output directory to retry; do not edit a failed receipt into a success.

Synthetic tests for this module mock the process backend. They neither execute
Cargo/native recovery/Wasm/Docker nor establish actual production readiness.
