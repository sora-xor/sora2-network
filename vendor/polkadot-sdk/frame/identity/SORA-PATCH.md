# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/identity`.

The earlier local source differed only by formatting. This copy includes the upstream stable2609 remove_registrar call, event/error additions and tests. Existing call indexes are retained. The new RegistrarRemoved event is appended rather than inserted after RegistrarAdded, preserving all 22 deployed event discriminants; a regression binds every legacy index plus the new encoded event. The new call/event API is additive and must be recorded by runtime metadata verification.

`Cargo.toml.orig` contains the exact upstream manifest. `Cargo.toml` materializes
upstream package, dependency and lint inheritance; SDK path dependencies are
pinned to the same release tag. Documented security overrides remain explicit.

[`stable2609-customizations.patch`](stable2609-customizations.patch) reproduces
the current Rust source, local formatter configuration and retained license copy
from this upstream directory. Its byte-for-byte replay was checked in an isolated
copy. [`stable2609-customizations.json`](stable2609-customizations.json) records
the patch hash and every changed source file hash. Earlier patch descriptions
below are historical rationale; their older base tags are not the current source.
