# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/benchmarking`.

The actual stable2609 benchmarking sources are adopted, retaining the maintained pastey package under the upstream paste dependency alias.

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

### SORA Nexus macro maintenance migration

Source: paritytech/polkadot-sdk, tag `polkadot-stable2603-7`, commit
`cd87d0b95e54c8ffc374334386f3f2b2c7a61f54`.

The `paste` dependency now names maintained `pastey` 0.2.3 under its
existing source alias. Runtime algorithms, layouts and generated identifiers
are unchanged. Workspace dependencies were materialized for this snapshot.
