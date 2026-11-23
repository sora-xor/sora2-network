# SORA Nexus stable2609 dependency migration

Upstream: `smoldot` 2.2.0, published crates.io archive.
Archive SHA-256: `7998fa4206a99d310bd288df99300724da1d7800f06b64387e431d9cd0d870f1`.
Upstream Git commit: `90e94869a7fbd617d28990da3005eaa906bc3862`.

The full upstream source was rebased to the version required by stable2609 and
its tooling. Archive checksums were checked against the registry lockfile entries.
The reviewed `sora-secp256k1-compat` dependency alias and ruzstd 0.9.1 are retained.
The final-block decoded-output limit fix, excessive-window and output-bound
regressions are preserved by a clean three-way source merge. The published crate uses `include = ["**/*.rs"]`, so its non-Rust test and
benchmark data is omitted. All 22 literal fixture inputs are now restored or
reverified byte-for-byte from the exact package Git commit above: 17 newly restored
unit-test inputs, three newly restored benchmark inputs, and the two existing
Zstandard runtime fixtures. The release `lib/Cargo.toml` independently confirms
version 2.2.0. No fixture data was substituted and no tests were disabled.
Upstream now declares the required hex and ed25519-zebra alloc features itself.

The local text changes are reproducible using `stable2609-customizations.patch`
against the archive above. `stable2609-customizations.json` records the patch
replay result and per-file checksums. Its `restoredFixtures` list binds every
non-Rust fixture to its release Git path, Git blob ID, size and SHA-256; these
fixtures are restored separately from the text patch because the crate archive
does not contain them. The regression harness records all 22 fixture identities.
The root workspace lockfile governs resolution;
the archive’s package-local lockfile is omitted.

Validation of the updated source is recorded in the stable2609 migration report;
the older results below apply to their original package versions only.

## Previous review rationale

### Earlier dependency migration

Upstream: `smoldot` 0.19.4, published crates.io archive.
Archive SHA-256: `e16e5723359f0048bf64bfdfba64e5732a56847d42c4fd3fe56f18280c813413`.
Upstream Git commit: `9446ff27f73be86fbf5ba3b3da57dabc1bff6bc2`.

Migrate the ruzstd consumer to maintained 0.9.1 APIs, removing vulnerable or unsound transitive versions.
The package version identifies the original consumer source; the dependency version is not relabeled.

Restore the two unchanged upstream decompression test fixtures omitted from the published archive, using the Git commit above (`lib/src/executor/host/zstd/`). SHA-256:

- `example-runtime.wasm.zstd`: `1e654f397605e50759773d3cb7e2609fcce2eb7de37b8213ccc093b01a2e47fe`
- `polkadot-runtime-v9160.wasm.zstd`: `ebfeb01e3fd0177c2d80de60d90a66b89b7a8fc83822094ba470afec34a0a111`

Declare required `alloc` dependency features explicitly instead of relying on the surrounding workspace: hex formatting/decoding and ed25519-zebra batch verification require allocation even in the no-default-features build.

Add an excessive-window regression for a valid nine-byte Zstandard frame that requests a multi-terabyte history window. Also enforce the decoded size in release builds: `UptoBytes` stops only between blocks, so a final raw block can exceed the requested maximum. The new boundary regression reproduces the previous debug assertion failure and release-mode limit bypass.

Validation: isolated no-default-features smoldot/smoldot-light build passes with ruzstd 0.9.1 and lru 0.19.0; all 6 decompression tests pass, including both restored runtime fixtures and the new window/output boundary regressions.

Retire libsecp256k1 in signing and recovery host functions through the new
`sora-secp256k1-compat` dependency alias. All cryptography now uses maintained
k256, including the versioned host calls that retain overflowing-scalar parsing.
No host-function source, version number, or signature encoding is changed. The
adapter records fixed fixtures and 14,094 differential compatibility checks in
its README; smoldot integration validation follows the combined maintenance migration.
