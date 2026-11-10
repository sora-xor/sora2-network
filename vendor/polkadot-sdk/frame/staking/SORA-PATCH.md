# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/staking`.

The additional payout hook, VAL replacement-reward compatibility, deferred-slash timing and rollback/weight regression tests are preserved. EraPayout now uses the upstream sp_staking trait; the obsolete ExposureOf type is removed as upstream requires. Storage version remains 16. Upstream marks this synchronous pallet deprecated in favor of pallet-staking-async; adopting async staking is a separate state/economic migration, not performed implicitly by this SDK source update.

`Cargo.toml.orig` contains the exact upstream manifest. `Cargo.toml` materializes
upstream package, dependency and lint inheritance; SDK path dependencies are
pinned to the same release tag. Documented security overrides remain explicit.

[`stable2609-customizations.patch`](stable2609-customizations.patch) reproduces
the current Rust source, local formatter configuration and retained license copy
from this upstream directory. Its byte-for-byte replay was checked in an isolated
copy. [`stable2609-customizations.json`](stable2609-customizations.json) records
the patch hash and every changed source file hash. Earlier patch descriptions
below are historical rationale; their older base tags are not the current source.

## Earlier patch rationale

### SORA Nexus compiler maintenance

The 2026-10-25 Clippy cleanup replaces deprecated primitive maximum-value
functions in tests/benchmarks with identical constants, preserves descending
test-account sorting with Reverse keys, and replaces an identity Option map
with unwrap_or(false). Staking amounts, vote scaling, and storage are unchanged.
