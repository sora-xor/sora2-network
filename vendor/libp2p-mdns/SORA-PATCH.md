# SORA Nexus dependency security migration

Based on `libp2p-mdns 0.46.0` from crates.io. Original archive SHA-256: `14b8546b6644032565eb29046b42744aee1e9f261ed99671b2c93fb140dba417`. The package version is retained to preserve the Polkadot SDK stable2603 API, while its dependencies are migrated to supported upstream versions. This is a source patch, not an advisory exemption.

Backport only Hickory 0.26 DNS message and resource-record field access from upstream 0.49.0; retain the original swarm/core APIs and address translation. Upgrade hickory-proto to 0.26.3 to fix RUSTSEC-2026-0119. The original mDNS packet construction/peer extraction tests exercise the migrated decoder.

Adapter reference: https://crates.io/crates/libp2p-mdns/0.49.0

Reference archive SHA-256: `9e82fb6db53f8ba1642bc0695ee589f7bcc1ff9028a9d068a88ee0db5b94667f`.

The migrated dependencies require Rust 1.88; the package metadata reflects that requirement.

Validation: all 5 library tests pass with the tokio feature.
