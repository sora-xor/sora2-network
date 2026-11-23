# SORA Nexus maintained Wasm tooling migration

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`,
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`. Original licenses
and notices are retained. Workspace manifests are materialized with the same
SDK revision.

Use wasmparser/wasm-encoder 0.262.0 to locate the required runtime_version custom section after compilation. Full reencoding parses every executable section entry and function instruction; merely iterating parser payloads would accept malformed bodies and weaken the previous deserialization check. Keep optional name/debug custom metadata opaque and retain the existing missing-section/build-failure policy without introducing stricter semantic validation. No compiler flags or production runtime version are changed by this parser migration.

Five permanent regression tests cover malformed instructions and section entries after metadata was already found, opaque invalid name metadata, missing runtime_version, and the existing distinction between parsing and semantic type validation.

[RUSTSEC-2022-0061](https://rustsec.org/advisories/RUSTSEC-2022-0061.html)
recommends the maintained [wasm-tools](https://github.com/bytecodealliance/wasm-tools)
project. No old parser implementation is copied or relabeled, and no advisory is suppressed.

The October 25 compiler-warning cleanup also supports Cargo's current build-script
layout: the invocation timestamp is now a sibling `run/invoked.timestamp` of
`OUT_DIR`, confirmed by the production build's `run/root-output` file. Prefer
`OUT_DIR/../run/invoked.timestamp`, falling back to the original
`OUT_DIR/../invoked.timestamp` only on `NotFound`. Other I/O errors remain visible;
the bloaty and compressed/compact binaries still receive the invocation's mtime.
Five regression tests cover both layouts, current-layout precedence, missing
timestamps, and propagation of non-missing I/O errors. This repairs timestamp
bookkeeping without changing the emitted Wasm contents or compilation flags.

## Stable 2609 rebase

Rebased on the exact upstream release above, retaining the previously reviewed
local source and dependency changes. `SORA-2609.patch` records only local Rust
source differences from this release; upstream source changes are included.
`SORA-2609-manifest-overrides.json` records the deliberate manifest policy
differences after workspace inheritance and SDK Git pins are materialized.
Validation is recorded in the root migration report, not inferred from this
source provenance.

The upstream Cargo JSON-target-spec detection for RISC-V is retained alongside
the local parser and invocation-timestamp fixes. The production Wasm target
continues through the ordinary Wasm build path.
