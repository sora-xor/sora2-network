# SORA Nexus dependency maintenance patch

Upstream: crates.io `ark-ff-0.5.0`.
Published crate SHA-256: `a177aba0ed1e0fbb62aa9f6d0502e9b46dad8c2eab04c14258a1212d2557ea70`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.
