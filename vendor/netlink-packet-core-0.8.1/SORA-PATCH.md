# SORA Nexus dependency maintenance patch

Upstream: crates.io `netlink-packet-core-0.8.1`.
Published crate SHA-256: `3463cbb78394cb0141e2c926b93fc2197e473394b761986eca3b9da2c63ae0f4`.

The original crate version is retained; only consuming dependencies and their
macro integration are migrated. This is not a republished replacement for an
unmaintained crate.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.
