# SORA Nexus build verification fix — 2026-10-25

The stale-Wasm attestation finding from the [live-version review](live_to_137_review_2026-10-25.md) is resolved. The hardened production build and all dependent verification passed. The regenerated runtime is byte-for-byte identical to the reviewed spec 137 / transaction 133 candidate.

## Fix

The build runner now removes every inherited Wasm skip control, including per-package flags and `DOCS_RS`, regardless of their values. It clears inherited compiler, target, profile, and nested-build overrides, then explicitly pins the host and Wasm toolchains to `rust-toolchain.toml`, the reviewed import flag, release mode, LLVM settings, and the existing warm Cargo target directory. Evidence records removed variable names without exposing their values.

Each execution gets a unique `FORCE_WASM_BUILD` value. Immediately before the production build, the runner archives the generated manifest and both compact Wasm exports. Both exports must be removed because the SDK can reuse either one. The runner requires all three files to be recreated before copying the candidate; compiler caches and the bloaty Wasm remain available. A skipped/no-op build can no longer certify a previous export. Optimized Python is rejected before build subprocesses run.

The independent finalizer checks the controlled environment, toolchain-file hash, runner/helper/test hashes, regression logs, unique build identifier, archived outputs, recreation timestamps, and candidate bytes. It rejects the old proof format that lacks these controls.

Changed implementation:

- [Build runner](../../runtime-rehearsal-async-staking-2026-10-25/validation/run-build-verification.py).
- [Environment policy](../../runtime-rehearsal-async-staking-2026-10-25/validation/build_environment.py).
- [Environment regressions](../../runtime-rehearsal-async-staking-2026-10-25/validation/test-build-environment.py) and [export/runner regressions](../../runtime-rehearsal-async-staking-2026-10-25/validation/test-build-verification.py).
- [Independent finalizer](../../runtime-rehearsal-async-staking-2026-10-25/validation/finalize-verification.py).

## Verification

| Check | Result |
| --- | --- |
| New regression suite | 20 tests passed, including child-process skip cases, stale/missing exports, warm-cache preservation, nonproduction manifests, and optimized Python |
| Actual build with hostile inherited settings | Passed with all three skip flags plus invalid nested toolchain/arguments/flags deliberately supplied to the parent environment; the recorded policy removed or replaced them |
| Full Clippy and formatting | All three Clippy configurations and formatting passed |
| Export recreation | Generated manifest, compact Wasm, and compressed Wasm recreated and hash-bound |
| Candidate identity | SHA-256 `535b6499c78769a62701520d0bdf17ef65f07319450ca69a5264e99410e1910f`, 1,959,169 bytes; unchanged |
| Independent proof mutation checks | Actual proof accepted; all six altered-proof cases rejected; 36 reviewed inputs unchanged |
| Fresh production Wasm rehearsals | All 24 stages passed, run `20261025T131656Z-1fee32e0` |
| Rejected admissions | Independent checks refreshed for 28 rejected transactions and 69 raw diff entries; 11 negative/rollback regressions passed |
| Fresh bridge state | Finalized block 28,191,008 checked; readiness evidence passed without asserting operator deployment |
| Final aggregation | Passed; candidate, source, native, audit, migration, and execution evidence bindings verified |

The original source fingerprint was explicitly reused only after fresh source-only checks before and after the build. Its file bytes and compiled-source aggregate remain unchanged. The earlier **943 passing Rust test executions**, three existing ignored tests, native production migration replay, and dependency audits retain their original execution dates and hashes. They were not rerun or counted as fresh executions for this tooling-only fix. New tooling is bound separately in the build evidence.

Prior build reports, logs, candidate, and final summary are archived under `validation/attempts/before-build-environment-fix-20261025T130526Z`; the rehearsal runners independently archive superseded execution results. No runtime source changes, publication, activation, or cleanup transaction was performed.

The [final summary](../../runtime-rehearsal-async-staking-2026-10-25/validation/rehearsal-summary.json) has SHA-256 `6e34a7276e1ca733d9069bfacc1edeb1b42ae0bcbe320f4fdb348b05238dd379`. The [build proof](../../runtime-rehearsal-async-staking-2026-10-25/validation/candidate-build-evidence.json) records build ID `20261025T131233Z-09828f72b5f242e489db42175801a52d`. The [independent mutation review](../../runtime-rehearsal-async-staking-2026-10-25/validation/build-policy-independent-review.json) records the six rejected proof mutations separately from test-suite counts.
