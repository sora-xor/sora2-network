# SORA Nexus Subxt macro maintenance migration

Source: crates.io `subxt-macro` **0.50.3**.
Published archive SHA-256:
`201ba29c4ad3cb5b387844bcc50f41467bc9f6f6c28aab1e4648b2c944cac762`.
The source is rebased onto the actual release. Its new unique-type-path handling
and other code-generation changes are retained.

Remove unmaintained `proc-macro-error2` (RUSTSEC-2026-0173). Diagnostics use
`Result`, maintained `syn::Error`, and emitted compiler error tokens directly.
This reapplies the reviewed local diagnostic mechanism to the new API; no panic
hook or unmaintained implementation is copied. Optional runtime-Wasm metadata
loading uses the same pinned Polkadot SDK stable2609 as the consuming node, avoiding
an obsolete parallel executor graph. The retired `no_mangle_generic_items` manifest
lint is removed because the compiler enforces that restriction as a hard error.

No advisory is suppressed. Source provenance does not substitute for validation;
compile, generation and diagnostic checks are recorded in the migration evidence.
