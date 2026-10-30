# SORA Nexus Wasmtime cache compression security migration

Original published consumer: `wasmtime-internal-cache 36.0.20`, upstream commit `b265f78b1e92e12240ec7bdc21c7a559d2df968e`, path `crates/cache`. `SORA-UPSTREAM.json` records the crates.io archive SHA-256, original file hashes and the local changes. The package retains its actual published identity.

Re-vendored from 36.0.19 for Wasmtime 36.0.20, which fixes RUSTSEC-2026-0424. Wasmtime requires this crate at exactly its own version, so the 36.0.19 copy no longer patches it. The published 36.0.20 package differs from 36.0.19 only in its version and VCS commit, and still requires `zstd` 0.13.0. The two local changes below carry over unchanged.

The dependency change is the manifest requirement on the genuine published `zstd` crate, from 0.13.0 to 0.14.3. It brings `zstd-safe 8.2.0` and `zstd-sys 2.1.2`, containing the October 2026 security fixes. The original default-features=false setting, cache encoding APIs and licenses remain unchanged. No compatibility shim or vulnerable crate is relabeled and no advisory is ignored.

`SORA-COMPAT.patch` separately records two source spelling updates from deprecated `usize::max_value()` to the equivalent `usize::MAX` constant in the worker queue-size clamp. The strict Clippy gate on the pinned October 2026 toolchain requires this change. The clamp value and control flow are unchanged; all other Rust source bytes match the published package. The original hashes remain in `SORA-UPSTREAM.json` to make that distinction verifiable.

This package belongs to the host Wasmtime executable cache. It does not execute in the production runtime Wasm. Cache compatibility, executor smoke tests, historical replay and the fresh root-lock audit are separate required validation. Existing immutable runtime artifacts and deployed legacy source exports remain untouched.

Upstream security release: https://github.com/gyscos/zstd-rs/releases/tag/v0.14.3
