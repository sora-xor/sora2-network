# SORA Nexus bridge coverage

Run from the repository with the pinned Rust toolchain, `cargo-llvm-cov` and its
matching `llvm-tools-preview` component installed:

```sh
scripts/bridge_coverage.sh
```

The command runs native library tests with the default features and requires
**strictly greater than 99% executable line coverage for each of 18 components**.
An aggregate percentage cannot hide an uncovered component, and a component
without measured lines fails. The scope includes `eth-bridge`, `bridge-proxy`, the
legacy bridge multisig, and all 15 pallets in `vendor/sora2-common/pallets`.
It does not measure node/RPC server integration, runtime dispatch integration,
`try-runtime`, runtime benchmarks, or deployed bridge contracts.

Reports are written to `target/bridge-coverage-reports/`: `index.html` links to
annotated source pages; `summary.json` contains per-component totals and uncovered
lines; `root.lcov`, `shared.lcov`, `root.json`, and `shared.json` retain the raw LLVM
evidence. Coverage from both workspaces is merged by source path and line number;
a line covered by either suite is covered once in the final denominator.

The denominator excludes test modules, mocks, fixtures, generated weight tables,
and benchmark harnesses. Inline tests use `coverage(off)` only when coverage
instrumentation is enabled. The handwritten BEEFY light-client weight functions
are included. Production migration and recovery modules are included.

There is one additional explicit generated-code exclusion. FRAME generates
instance methods on `Call<T>` even for pallets with no dispatchable calls. The
outbound channel and leaf provider have no such calls; the only enum variant
contains `Never`, so these instance methods cannot be invoked safely. LLVM maps
their unreachable bodies onto unrelated source declarations. The reporter uses
LLVM JSON function/code-region provenance to remove only zero-hit lines belonging
exclusively to seven recognized generated instance methods in those two files.
It retains any line that also belongs to a real function, even an uncovered one,
and retains callable static metadata methods. It rejects this exclusion if a
pallet gains dispatchables, adds handwritten methods with those names, or a
supposed unreachable instance method executes. Every removed line and symbol is
listed in `summary.json`; raw LLVM reports remain unfiltered. The generator is
`vendor/polkadot-sdk/frame/support/procedural/src/pallet/expand/call.rs`.

The persistent build lane is `target/bridge-coverage`, overridable with
`CARGO_LLVM_COV_TARGET_DIR`. At the start of an authoritative run, the runner
cleans only the explicitly measured packages in both workspaces and clears old
raw execution profiles and merged profile data. Third-party dependencies and the
ordinary build directory stay warm. This targeted rebuild matters because `cargo-llvm-cov --no-report`
implies `--no-clean`, and report generation scans workspace test executables:
old executables can otherwise contribute obsolete source mappings. Do not run
another build or coverage run in this lane concurrently.

The runner records SHA-256 hashes of the scoped source trees (including tests and
fixtures), their local common/assets/technical/permissions dependency sources,
package manifests, both workspace manifests/lockfiles, Cargo config,
the toolchain file, and coverage scripts before and after testing. It fails if
those inputs change during the run. Both hash manifests are retained beside the
reports. Freeze source edits until the run finishes.

For reporter regression checks alone:

```sh
python3 -m unittest discover -s scripts -p test_bridge_coverage.py
```

`bridge_coverage.py --no-gate` is available for inspecting partial development
runs. A development report does not replace a successful full runner invocation.
