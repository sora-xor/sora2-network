# SORA Nexus dependency migration

Upstream: `libp2p-swarm` 0.45.1, published crates.io archive.
Archive SHA-256: `d7dd6741793d2c1fb2088f67f82cf07261f25272ebe3c0b0c311e0c6b50e851a`.
Upstream Git commit: `87101870709aaf64e141e07624855155ed4faaac`.

Migrate the lru consumer to maintained 0.19.0 APIs, removing vulnerable or unsound transitive versions.
The package version identifies the original consumer source; the dependency version is not relabeled.
