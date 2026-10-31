# SORA Nexus dependency security patch

Upstream: crates.io `alloy-primitives` 1.5.7. Published archive SHA-256:
`de3b431b4e72cd8bd0ec7a50b4be18e73dab74de0dba180eef171055e5d5926e`. Source and license files are retained.

Use k256 0.14; adapt SEC1 point conversion used for Ethereum addresses. Existing signature and address fixtures provide compatibility validation.

Changed source files: `src/bits/address.rs`. Also update the dependency manifest.

The actual stable RustCrypto releases select elliptic-curve 0.14 and the
workspace-locked crypto-bigint 0.7.6, fixing
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html)
([upstream release](https://github.com/RustCrypto/crypto-bigint/pull/1365)).
Keep maintained lockfiles at crypto-bigint 0.7.6 or later. No vulnerable package
is relabeled and no advisory is suppressed. Compatibility tests do not constitute
a new constant-time proof of upstream arithmetic.

Validation with k256 and arbitrary features: 12 signature tests pass, including non-normalized Ethereum recovery and Web3 fixtures.

Tests were run in an isolated workspace using the retained root lockfile versions
and these vendored source files. This avoids pulling unrelated newer Alloy
releases into the verification graph.

The published crate's development Cargo.lock is omitted: this vendored library
uses the authoritative root lockfile and is not an independently locked project.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.
