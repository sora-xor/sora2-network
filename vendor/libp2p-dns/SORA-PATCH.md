# SORA Nexus dependency security migration

Based on `libp2p-dns 0.42.0` from crates.io. Original archive SHA-256: `97f37f30d5c7275db282ecd86e54f29dd2176bd3ac656f06abf43bedb21eb8bd`. The package version is retained to preserve the Polkadot SDK stable2603 API, while its dependencies are migrated to supported upstream versions. This is a source patch, not an advisory exemption.

Backport the Hickory 0.26 resolver builder and typed answer access from upstream 0.45.0, while preserving libp2p-core 0.42 transport types and dial limits. Use hickory-resolver 0.26.3 (and hickory-net/proto 0.26.3), fixing RUSTSEC-2026-0118 and RUSTSEC-2026-0119. An offline regression exercises A/AAAA/TXT mapping and filters CNAME/unrelated TXT answers.

Adapter reference: https://crates.io/crates/libp2p-dns/0.45.0

Reference archive SHA-256: `18694695e98ff9ce26ee245fdede7cd0b4186cb793b25da3258d9c053400a291`.

The migrated dependencies require Rust 1.88; the package metadata reflects that requirement.


Retire async-std and async-std-resolver entirely. The new smol transport uses smol 2.0.2 networking, timers, background tasks, and blocking-I/O support. The historical async-std feature/module names forward to this implementation for SDK source compatibility; no async-std code or package remains in its dependency graph. Runtime adapter source derives from async-std-resolver 0.24.4, original archive SHA-256 `b766684a183c8a439f2ca24d6973cf8bfdc1353ae9c54b2b91e81d2630dd034e`; preserve its MIT/Apache license files. Maintain bind addresses, TCP connect deadlines, unconnected UDP behavior, and detached background tasks. Add offline socket, timer, spawn, and resolver tests.

Validation: all-feature compilation and five offline tests pass, covering DNS record conversion, literal-IP resolution, TCP/UDP loopback, timeout and background spawning. The separate public-DNS test is excluded from the offline run.
