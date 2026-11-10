# SORA Nexus dependency security patch

Upstream: crates.io `alloy-consensus` 1.6.3. Published archive SHA-256:
`4e4ff99651d46cef43767b5e8262ea228cd05287409ccb0c947cc25e70a952f9`. Source and license files are retained.

Use k256 0.14; adapt infallible recoverable signing, SEC1 point APIs and signature normalization while retaining recovery-ID parity handling. Adapt arbitrary/test generators to rand 0.10 while retaining unrelated public RNG APIs. Existing Ethereum signing/recovery suites provide compatibility validation.

Changed source files: `src/signed.rs`, `src/crypto.rs`. Also update the dependency manifest.

The actual stable RustCrypto releases select elliptic-curve 0.14 and the
workspace-locked crypto-bigint 0.7.6, fixing
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html)
([upstream release](https://github.com/RustCrypto/crypto-bigint/pull/1365)).
Keep maintained lockfiles at crypto-bigint 0.7.6 or later. No vulnerable package
is relabeled and no advisory is suppressed. Compatibility tests do not constitute
a new constant-time proof of upstream arithmetic.

Validation with k256, secp256k1, arbitrary and serde features: 3 crypto tests pass, including agreement between RustCrypto and native secp256k1.

Tests were run in an isolated workspace using the retained root lockfile versions
and these vendored source files. This avoids pulling unrelated newer Alloy
releases into the verification graph.

The published crate's development Cargo.lock is omitted: this vendored library
uses the authoritative root lockfile and is not an independently locked project.
