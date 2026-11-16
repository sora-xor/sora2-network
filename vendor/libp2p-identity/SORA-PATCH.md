# SORA Nexus dependency security patch

Upstream: crates.io `libp2p-identity` 0.2.13. Published archive SHA-256:
`f0c7892c221730ba55f7196e98b0b8ba5e04b4155651736036628e9f73ed6fc3`. Source and license files are retained.

Use k256/p256 0.14 and sec1 0.8.1; adapt SEC1 point APIs, p256 SecretKey DER encoding, Generate and rand 0.10. Normalize high-S secp256k1 signatures before verification to preserve the previous accepted signature set. Reject incorrect digest lengths without panicking. Restore the quickcheck development dependency omitted from the published manifest. Added fixed vectors captured using unmodified k256 0.13.4 and p256 0.13.2 covering signature bytes, high-S verification, public keys, DER keys, malformed input and secret-buffer zeroization.

Changed source files: `src/secp256k1.rs`, `src/ecdsa.rs`. Also update the dependency manifest.

The actual stable RustCrypto releases select elliptic-curve 0.14 and the
workspace-locked crypto-bigint 0.7.6, fixing
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html)
([upstream release](https://github.com/RustCrypto/crypto-bigint/pull/1365)).
Keep maintained lockfiles at crypto-bigint 0.7.6 or later. No vulnerable package
is relabeled and no advisory is suppressed. Compatibility tests do not constitute
a new constant-time proof of upstream arithmetic.

All-feature validation: 23 unit tests, 1 keypair API test, 4 serialization tests and 1 doctest pass.

Tests were run in an isolated workspace using the retained root lockfile versions
and these vendored source files. This avoids pulling unrelated newer Alloy
releases into the verification graph.

The published crate's development Cargo.lock is omitted: this vendored library
uses the authoritative root lockfile and is not an independently locked project.
