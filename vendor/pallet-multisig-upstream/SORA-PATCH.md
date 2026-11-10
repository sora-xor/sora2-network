# SORA Nexus stable2609 Multisig migration

This is the actual `pallet-multisig` 50.0.0 source from
`polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, directory
`substrate/frame/multisig`. The package remains named
`pallet-multisig-upstream` to distinguish the runtime's upstream-derived
Multisig pallet from the separately customized bridge Multisig pallet.

`Cargo.toml.orig` is the exact upstream manifest. `Cargo.toml` materializes
upstream workspace package fields, dependencies and lint policy, pins SDK
Git dependencies to the release tag, and preserves the package alias.
`stable2609-customizations.patch` reproduces the local manifest and source
changes. Its SHA-256, per-file checksums and successful isolated replay check
are recorded in `stable2609-customizations.json`.

## Preserved customizations

The empty `GenesisConfig` and `BuildGenesisConfig` retain the existing runtime
genesis JSON shape. They perform no storage writes. The prior `Debug` derive
migration is now upstream. The blanket legacy Clippy suppression was removed;
the manifest carries the current upstream lint policy. The logging macro uses
`$crate::LOG_TARGET`, and the constant uses an elided static lifetime. A local
rustfmt configuration retains the SDK's source formatting.

## Source and state review

The original local snapshot was compared against the published
`pallet-multisig` 45.0.0 archive (SHA-256
`bb5c61c65c78314405af4dd939e780f8ed107a2a3a6b660dc44efa5e1076f2cb`,
Git commit `ac2b0e3fd86a23cc33b2f4d6f46ac9d0ecb05115`). Apart from formatting,
the local Rust delta was the empty genesis configuration, the `Debug` derives,
and broad Clippy attributes. The genesis behavior is retained; no customized
dispatch or fee behavior was replaced.

The in-code storage version stays at 1. `src/migrations.rs`, the multisig
storage layout, call indices and parameters, error/event variant order and
fields, and dispatch algorithms are unchanged. The new upstream release
clarifies approval versus execution and adds regression tests for timepoints,
wrong signatories/hashes, signatory ordering, sender inclusion and repeated
approvals. No additional persistent storage migration is required by this
pallet update. This static review complements runtime metadata and production
state rehearsals; it does not claim those execution gates have passed.
