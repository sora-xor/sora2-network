# SORA Nexus Wasm interface security floor

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`,
`substrate/primitives/wasm-interface`.

All upstream Rust source and license notices are retained. In particular, the
new mutable `FunctionContext::read_memory` and `read_memory_into` receivers are
preserved for the stable2609 executor. The manifest materializes upstream workspace metadata, dependency
features and lint policy with the exact stable2609 Git tag.

The Wasmtime requirement remains `36.0.20` with default features disabled,
raising the upstream `36.0.7` minimum to the already reviewed security floor.
This is a dependency floor, not a relabeled package or advisory suppression.
The lockfile must preserve one compatible Wasmtime family and the exact runtime
candidate must pass the node/executor and production-state rehearsals.

`SORA-2609-manifest-overrides.json` records this intentional dependency delta.
`SORA-2609.patch` records Rust differences, if any, from the upstream snapshot.
Validation results belong to the root migration report.

The 36.0.20 floor includes [GHSA-6fr9-xmw9-5352](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-6fr9-xmw9-5352), released October 29, 2026. The existing feature selection is unchanged.
