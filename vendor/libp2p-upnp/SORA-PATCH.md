# SORA Nexus dependency security migration

Based on `libp2p-upnp 0.3.0` from crates.io. Original archive SHA-256: `01bf2d1b772bd3abca049214a3304615e6a36fa6ffc742bdd1ba774486200b8f`. The package version is retained to preserve the Polkadot SDK stable2603 API, while its dependencies are migrated to supported upstream versions. This is a source patch, not an advisory exemption.

Migrate igd-next from 0.14.3 to 0.18.0, which uses the maintained hyper 1.x / h2 0.4 stack instead of hyper 0.14 / h2 0.3 (RUSTSEC-2026-0258). No libp2p swarm behavior/API changes. Check with the tokio feature enabled.

Adapter reference: https://crates.io/crates/igd-next/0.18.0

Reference archive SHA-256: `c71312255fdc5364bff7eec0adf68146bc4e72aba85075261af40d234c11be99`.

The migrated dependencies require Rust 1.88; the package metadata reflects that requirement.
