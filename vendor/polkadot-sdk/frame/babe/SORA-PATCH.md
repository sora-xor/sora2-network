# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/babe`.

The signed equivocation fee policy, rejection of all unsigned reports and disabled offchain report publication are retained. Existing report call indexes and proof encodings remain unchanged. The upstream Offence associated type is now `Slot`; legacy unsigned admission adapters use the upstream narrowly scoped deprecation allowance. The epoch test helper retains its previously reviewed boundary behavior.

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

### SORA Nexus equivocation report fee policy

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2603-3`,
commit `e3737178ec726cffe506c907263aaaa417893fd0`,
source directory `substrate/frame/babe`. The Apache 2.0 license and
original file notices are retained. Workspace dependencies and lint settings
are made explicit for the SORA Nexus workspace, following the existing staking fork.

Local policy:

- Successful `report_equivocation` calls retain `Pays::Yes`.
- The existing `report_equivocation_unsigned` name, SCALE call index and proof
  arguments are retained as a compatibility alias. It now requires a signed
  origin, uses that signer as the reporter, and retains `Pays::Yes`.
- Unsigned transaction validation and pre-dispatch always reject with
  `InvalidTransaction::Call`, including local and in-block submissions.
- `submit_unsigned_equivocation_report` returns `None` without publishing
  evidence or making offchain host calls. Reports must be submitted as signed
  transactions through either report call.
- Proof validation, offence processing, duplicate rejection and report weights
  retain the upstream behavior.

Unit tests cover both signed call variants, retained successful and duplicate
report fees, rejection of every unsigned transaction source, unsigned origin
rejection, and the disabled publication helper.

The 2026-10-25 compiler cleanup makes a test assertion a statement and uses
iterator for_each in the test helper, retaining the original order of block
progression followed by slot increment (including overflow timing). Consensus
production code is unchanged.
