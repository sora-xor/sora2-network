# SORA Nexus dependency maintenance patch

Upstream: crates.io `tikv-jemalloc-ctl-0.6.1`.
Published crate SHA-256: `661f1f6a57b3a36dc9174a2c10f19513b4866816e13425d3e418b11cc37bc24c`.

The original crate version is retained. Consuming dependencies are migrated;
this is not a renamed copy of an unmaintained dependency.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.

The upstream packaged standalone Cargo.lock still referenced paste 1.0.15 after
the consumer manifest migration. The lock was regenerated against this manifest
on 2026-10-24, retaining the real crate versions and resolving pastey 0.2.3 with
current compatible transitive dependencies. Its eight-package graph is audited
separately with `cargo audit --deny warnings`; CI and Dependabot include this
standalone lock rather than relying only on the root workspace graph.

The focused `cargo check --locked --all-features --lib` passes, expanding the
unchanged consumer macros with pastey and building the native allocator dependency.
One existing `cargo-clippy` check-cfg compiler warning remains; it is unrelated
to memory safety or RustSec maintenance advisories.
