# SORA Nexus dependency maintenance patch

Upstream: crates.io `ark-ff-0.4.2`.
Published crate SHA-256: `ec847af850f44ad29048935519032c33da8aa03340876d351dfab5660d2966ba`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.

Replace abandoned `derivative` with maintained `derive-where` 1.7.0 using
its safe expansion mode. Explicit bounds and ignored Debug fields are retained;
Runtime algorithms and serialized representations are unchanged.
