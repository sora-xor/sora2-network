# SORA Nexus dependency maintenance patch

Upstream: crates.io `ark-ec-0.4.2`.
Published crate SHA-256: `defd9a439d56ac24968cca0571f598a61bc8c55f71d50a89cda591cb750670ba`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

Replace abandoned `derivative` with maintained `derive-where` 1.7.0 using
its safe expansion mode. Explicit bounds and ignored Debug fields are retained;
Runtime algorithms and serialized representations are unchanged.
