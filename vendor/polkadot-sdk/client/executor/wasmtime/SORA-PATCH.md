# SORA Nexus Wasmtime migration

Upstream: [`polkadot-stable2609`](https://github.com/paritytech/polkadot-sdk/tree/7a8daa68e18718c18823c1109513fdc1c8aa71fe/substrate/client/executor/wasmtime),
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`.
All upstream Rust source, build scripts and tests are copied without modification.
This includes the new per-instance heap strategy and pooled-instance updates;
the older compile-time memory-limit implementation is not retained.

The manifest materializes upstream package metadata, lint policy, workspace
dependency requirements and features. SDK dependencies use the same release tag;
`sp-wasm-interface` references the migrated local copy. Wasmtime's upstream
`36.0.7` floor is raised to the already reviewed security floor **36.0.20**.
The compatible rustix requirement and `use-libc` feature are preserved.
The unmaintained `paste` test dependency uses maintained `pastey` 0.2.3 under
its existing source alias. Package version 0.29.0 is the actual upstream version.

No security advisory is ignored. This file records source provenance; completed
build, audit and runtime execution results belong in the migration evidence.

The 36.0.20 floor includes [GHSA-6fr9-xmw9-5352](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-6fr9-xmw9-5352), released October 29, 2026. The existing feature selection is unchanged.
