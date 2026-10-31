# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/preimage`.

The signed requested-preimage replay fee policy and its regression cases are retained. Upstream Rust source is unchanged across these SDK releases; manifests now use the actual stable2609 crate version and graph, preserving explicit serde feature requirements.

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

### SORA Nexus preimage patch

Upstream: https://github.com/paritytech/polkadot-sdk.git
Tag: `polkadot-stable2603-3`
Commit: `e3737178ec726cffe506c907263aaaa417893fd0`
Path: `substrate/frame/preimage`

Rust sources originate from the exact resolved SDK checkout. Cargo workspace
metadata, lint policy and dependencies are expanded with the same versions and
SDK tag for this standalone vendored crate. Apache-2.0 license is retained.

SORA Nexus changes: an ordinary signed replay of an already supplied requested preimage is rejected with the existing
`AlreadyNoted` error, so signed replay failures retain transaction fees. First
requested provision keeps its success refund. Manager conversion of an unrequested
preimage to a requested preimage, repeated internal StorePreimage notes, request counters, deposits, storage layouts and
call encodings are preserved. Regression tests cover first provision, replay,
multiple outstanding requests, manager conversion and subsequent re-provision.
