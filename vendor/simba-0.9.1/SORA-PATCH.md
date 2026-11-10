# SORA Nexus dependency maintenance patch

Upstream: crates.io `simba-0.9.1`.
Published crate SHA-256: `c99284beb21666094ba2b75bbceda012e610f5479dfcc2d6e2426f53197ffd95`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.

The 2026-10-25 compiler-warning cleanup uses primitive floating-point associated
`MIN`/`MAX` constants instead of deprecated module constants. Values and the
`RealField` API remain identical.
