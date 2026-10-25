# SORA Nexus dependency maintenance patch

Upstream: crates.io `ark-ff-0.3.0`.
Published crate SHA-256: `6b3235cc41ee7a12aaaf2c575a2ad7b46713a8a50bda2fc3b003a04845c05dd6`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.

Replace abandoned `derivative` with maintained `derive-where` 1.7.0 using
its safe expansion mode. Explicit bounds and ignored Debug fields are retained;
Runtime algorithms and serialized representations are unchanged.
