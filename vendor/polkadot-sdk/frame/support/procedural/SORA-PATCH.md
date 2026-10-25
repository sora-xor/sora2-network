# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/support/procedural`.

The actual stable2609 procedural implementation is adopted. The local CloneNoBound generic helper, mixed-site identifier hygiene and uninhabited enum handling are preserved. The helper preserves observable field cloning without adding bounds on generic parameters or suppressing Clippy checks.

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

The existing vendored Polkadot SDK `frame-support-procedural` 37.0.0 source
retains its upstream license and crate identity. The 2026-10-25 Clippy cleanup
updates `CloneNoBound` to clone fields through a local generic `Field: Clone`
helper. A derive macro cannot resolve whether arbitrary field types implement
Copy; the generic operation is valid for both Copy and non-Copy fields and calls
the same Clone implementation exactly once per field in the original order.
No Clone bounds are added to the containing type's parameters. The helper is
omitted for fieldless structures/enums, and can be inlined by the compiler.
No Clippy lint is suppressed and source spans are not masked. Regression tests
cover named/tuple structures, named/tuple/unit enum variants, non-Clone marker
parameters, Copy fields, and a non-Copy field with observable Clone behavior.

Upstream source: Polkadot SDK tag `polkadot-stable2603-7`, exact commit
`cd87d0b95e54c8ffc374334386f3f2b2c7a61f54`, path
`substrate/frame/support/procedural`. The previously copied nested source is
activated by a Cargo patch. `Cargo.toml` resolves inherited upstream package,
dependency, and lint settings explicitly; SDK path dependencies retain the same
Git tag. The unchanged upstream manifest is retained as `Cargo.toml.orig`
(SHA-256 `e857d2396b09edbbf9c2337536d1f0bfda8e15310fa039ac19bed5c9fa859db1`).
The private helper identifier uses mixed-site hygiene so a user field with the
same spelling cannot shadow it.

The empty-enum regression exposed an upstream derive defect: matching an
empty enum through a reference is non-exhaustive. Its generated Clone body now
matches the uninhabited dereferenced value, as required by Rust exhaustiveness,
and emits no unused field helper. No valid inhabited value is affected.
