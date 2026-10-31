# SORA Nexus dependency maintenance patch

Upstream: crates.io `ark-poly-0.4.2`.
Published crate SHA-256: `d320bfc44ee185d899ccbadfa8bc31aab923ce1558716e1997a1e74057fe86bf`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

Replace abandoned `derivative` with maintained `derive-where` 1.7.0 using
its safe expansion mode. Explicit bounds and ignored Debug fields are retained;
SparsePolynomial equality continues to ignore num_vars through a direct impl.
