# SORA Nexus Zstandard security migration

Upstream: [`polkadot-stable2609`](https://github.com/paritytech/polkadot-sdk/tree/7a8daa68e18718c18823c1109513fdc1c8aa71fe/substrate/primitives/maybe-compressed-blob), commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`. Published package identity `sp-maybe-compressed-blob 11.0.0` is retained. `SORA-UPSTREAM.json` records the original file hashes; workspace metadata, dependencies and lint policy are materialized from that exact SDK revision.

The host Wasm compression/decompression dependency is migrated from zstd 0.12.4 to the published zstd 0.14.3 security release, which requires zstd-safe 8.2.0 and zstd-sys 2.1.2. Every Rust source file is unchanged. Compression limits, magic bytes, streaming APIs and feature flags are retained.

The dependency migration addresses the [zstd-rs 0.14.3 release](https://github.com/gyscos/zstd-rs/releases/tag/v0.14.3) and the October 2026 RustSec Zstandard advisories. No vulnerable compression implementation is relabeled, copied under a safe version or exempted from audit. This source is a node/build-time host dependency, not a production runtime no_std algorithm change.

The preserved deployed spec-134 source export and frozen recovery Wasm artifacts are not modified by this root-workspace patch. Compatibility, regenerated artifact hashes and independent execution must be verified before a new release binding. Validation is recorded separately; provenance alone is not a test result.
