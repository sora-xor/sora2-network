# SORA Nexus legacy type-name maintenance migration

Based on the checksum-verified crates.io `scale-info-legacy` 0.4.2 source archive.
Replaces the internal `SmallString<[u8; 16]>` name buffer with `alloc::String`,
removing unmaintained `smallstr` (RUSTSEC-2026-0215). Public type-name parsing,
comparison, hashing and serialization continue to use string contents. The
tradeoff is heap allocation for small internal names; no wire encoding changes.
The upstream parsing and type-registry tests remain in place. This library is
validated against the root workspace lockfile.
