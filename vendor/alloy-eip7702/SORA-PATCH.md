# SORA Nexus dependency security patch

Upstream: crates.io `alloy-eip7702` 0.6.3. Published archive SHA-256:
`2919c5a56a1007492da313e7a3b6d45ef5edc5d33416fdec63c0d7a2702a0d20`. Source and license files are retained.

Use k256 0.14 and its Generate trait with rand 0.10 for arbitrary authorization generation. Keep authorization encodings and signing semantics unchanged; retain existing recovery tests.

Changed source files: `src/auth_list.rs`. Also update the dependency manifest.

The actual stable RustCrypto releases select elliptic-curve 0.14 and the
workspace-locked crypto-bigint 0.7.6, fixing
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html)
([upstream release](https://github.com/RustCrypto/crypto-bigint/pull/1365)).
Keep maintained lockfiles at crypto-bigint 0.7.6 or later. No vulnerable package
is relabeled and no advisory is suppressed. Compatibility tests do not constitute
a new constant-time proof of upstream arithmetic.

Validation with k256 and arbitrary features: all 3 authorization tests pass.

Tests were run in an isolated workspace using the retained root lockfile versions
and these vendored source files. This avoids pulling unrelated newer Alloy
releases into the verification graph.

The published crate's development Cargo.lock is omitted: this vendored library
uses the authoritative root lockfile and is not an independently locked project.
