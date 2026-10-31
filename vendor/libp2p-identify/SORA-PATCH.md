# SORA Nexus dependency migration

Upstream: `libp2p-identify` 0.45.0, published crates.io archive.
Archive SHA-256: `1711b004a273be4f30202778856368683bd9a83c4c7dcc8f848847606831a4e3`.
Upstream Git commit: `4725f46f0d0e14f3ef4e88edc085729c1312dff6`.

Remove the unused lru dependency (no Rust code in this crate references it).
The package version identifies the original consumer source; the dependency version is not relabeled.
