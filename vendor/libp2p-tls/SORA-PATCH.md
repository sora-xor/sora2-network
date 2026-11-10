# SORA Nexus dependency security migration

Based on `libp2p-tls 0.5.0` from crates.io. Original archive SHA-256: `47b23dddc2b9c355f73c1e36eb0c3ae86f7dc964a3715f0731cfad352db4d847`. The package version is retained to preserve the Polkadot SDK stable2603 API, while its dependencies are migrated to supported upstream versions. This is a source patch, not an advisory exemption.

Backport certificate generation and webpki error adaptations from upstream 0.7.0, then migrate rcgen to 0.14.10 using PublicKeyData::subject_public_key_info. Retain the original ring backend, SDK core and identity types, protocol algorithms and authentication behavior. Require rustls-webpki >=0.103.16, fixing RUSTSEC-2026-0104, RUSTSEC-2026-0098, RUSTSEC-2026-0099 and RUSTSEC-2026-0382. Removing rcgen 0.11 also removes its ring 0.16 dependency (RUSTSEC-2025-0009). Includes upstream expired-certificate regression and original authenticated handshake tests.

Adapter reference: https://crates.io/crates/libp2p-tls/0.7.0

Reference archive SHA-256: `01dc85ad2bbc3506502f430a00dd913ec87f31e1894291cb9b923168babb6485`.

The migrated dependencies require Rust 1.88; the package metadata reflects that requirement.

The published crate excludes certificate fixtures. Restore its unchanged upstream fixtures from rust-libp2p commit `302b4c767c0c2274115bacded11eadc944de82cf`, directory `transports/tls/src/test_assets`, so all upstream certificate tests can run. SHA-256:

- `ed25519.der`: `c3a68d060b7bb96befec9ed779b49b73e601f02b82c071251f2f4e4c9d6696bc`
- `ed448.der`: `4eec81f97b4198bf4d6d7c97c12af2e6051737fdd57c92171c81a6401c9570fe`
- `nistp256_sha256.der`: `4a4bc24de8f24c3852f352168887e79fc87549803e75f1016f8cf84dfc6eab20`
- `nistp384_sha256.der`: `bfe8b782de80239f030bafb30c4d95e6ae934e6bfa7c2c7f59bfdcbedd7ab27a`
- `nistp384_sha384.der`: `945f464df6a713a20e3abe28a030ed772231871e234a8fe66ea68728447e232f`
- `nistp521_sha512.der`: `013d0ff6248010f8c55cf0ff4d686ac3f3df4249885de7399d1f772aa3d14c1b`
- `rsa_pkcs1_sha256.der`: `5b84fa4e4b90d831ed6cf50412d74859738c3dc34ca4617e66aa2d8f165372a4`
- `rsa_pkcs1_sha384.der`: `465d4ad5ae48c22a2b0e3646a0e3a3f7f692abf0755e35088267ac6cd3a82f5b`
- `rsa_pkcs1_sha512.der`: `bfa0b02ce2362afe3ed0da43c6ff6caa5f19fb3b9328efc329a442159dee33d7`
- `rsa_pss_sha384.der`: `1e0ba2b053338a78b4a53c76d3a811d68b420b34146f53f89f2dc41ba427ec70`

Validation: all 16 certificate unit tests and the authenticated two-peer transport handshake test pass on nightly-2026-10-23 in an isolated compatibility workspace with libp2p-core 0.42 / swarm 0.45.
