# SORA Nexus Zstandard security migration

Upstream: [`polkadot-stable2609`](https://github.com/paritytech/polkadot-sdk/tree/7a8daa68e18718c18823c1109513fdc1c8aa71fe/polkadot/node/primitives), commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`. Published package identity `polkadot-node-primitives 27.0.0` is retained. `SORA-UPSTREAM.json` records the original file hashes; workspace metadata, dependencies and lint policy are materialized from that exact SDK revision.

Remove the unused non-unknown-target zstd dependency. No Rust source file in this exact upstream package references zstd; compressed PoV handling uses sp-maybe-compressed-blob. All Rust source, other dependencies and features remain unchanged. The package version remains the actual upstream 27.0.0.

The dependency migration addresses the [zstd-rs 0.14.3 release](https://github.com/gyscos/zstd-rs/releases/tag/v0.14.3) and the October 2026 RustSec Zstandard advisories. No vulnerable compression implementation is relabeled, copied under a safe version or exempted from audit. This source is a node/build-time host dependency, not a production runtime no_std algorithm change.

The preserved deployed spec-134 source export and frozen recovery Wasm artifacts are not modified by this root-workspace patch. Compatibility, regenerated artifact hashes and independent execution must be verified before a new release binding. Validation is recorded separately; provenance alone is not a test result.
