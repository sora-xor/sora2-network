# SORA Nexus legacy recovery migration

Upstream: `sp-io` from `paritytech/polkadot-sdk` tag `polkadot-stable2609`,
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`.
Sources and license notices are retained. Workspace dependencies and lints are
made explicit; internal SDK dependencies retain that tag.

Replace both regular and native-target libsecp256k1 dependencies with the new
`sora-secp256k1-compat` package through the existing dependency alias. This uses
maintained k256 arithmetic and preserves the legacy v1 host functions' deliberate
overflowing scalar parsing, public key encodings, and recovery behavior. Native
v2 recovery continues to use the maintained secp256k1 crate.

The original Rust source and runtime-interface versions are unchanged. The new
adapter's README records the historical reference, archive hashes, fixed vectors,
and 14,094 differential checks. Integration must retain native and runtime Wasm
validation, since these host-function interfaces are protocol sensitive.

## Stable 2609 rebase

Rebased on the exact upstream release above, retaining the previously reviewed
local source and dependency changes. `SORA-2609.patch` records only local Rust
source differences from this release; upstream source changes are included.
`SORA-2609-manifest-overrides.json` records the deliberate manifest policy
differences after workspace inheritance and SDK Git pins are materialized.
Validation is recorded in the root migration report, not inferred from this
source provenance.
