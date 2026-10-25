# SORA Nexus maintained Wasm tooling migration

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`,
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`. Original licenses
and notices are retained. Workspace manifests are materialized with the same
SDK revision.

Replace parity-wasm with wasmparser/wasm-encoder 0.262.0. Runtime APIs retain their historical SCALE tuple encoding; runtime_version excludes API data. Replace only the first matching custom section and append absent sections in the same order as before. Custom sections, including optional debug/name payloads, remain opaque. Regression tests cover replacement, duplicate sections, byte preservation, append ordering and malformed instructions.

[RUSTSEC-2022-0061](https://rustsec.org/advisories/RUSTSEC-2022-0061.html)
recommends the maintained [wasm-tools](https://github.com/bytecodealliance/wasm-tools)
project. No old parser implementation is copied or relabeled, and no advisory is suppressed.

## Stable 2609 rebase

Rebased on the exact upstream release above, retaining the previously reviewed
local source and dependency changes. `SORA-2609.patch` records only local Rust
source differences from this release; upstream source changes are included.
`SORA-2609-manifest-overrides.json` records the deliberate manifest policy
differences after workspace inheritance and SDK Git pins are materialized.
Validation is recorded in the root migration report, not inferred from this
source provenance.

Upstream removal of `NativeVersion` and `GetNativeVersion` is preserved. The
node consumes runtime versions through the Wasm executor APIs.
