# SORA Nexus maintenance migration

Original consumer: `multihash 0.17.0`, crates.io archive SHA-256 `835d6ff01d610179fbce3de1694d007e500bf33a7f29689838941d6bf783ae40`. The package version identifies the original source; dependencies are actually replaced rather than relabeled or exempted from audit.

Replace the abandoned/yanked core2 crate with no_std_io2 0.9.4, the maintained fork explicitly recommended by RUSTSEC-2026-0105. Use its actual crate name and core::error::Error; preserve the multihash byte encoding and public types. The std path continues to use std::io. Declare small_rng on the existing test rand dependency so upstream property tests compile independently. MSRV is 1.81, matching no_std_io2. Reference: https://rustsec.org/advisories/RUSTSEC-2026-0105.html .

Validation: all-feature compilation, ten upstream library tests, and separate no-default-feature and alloc-only compilation checks pass.
