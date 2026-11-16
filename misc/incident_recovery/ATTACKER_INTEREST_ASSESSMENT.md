# SORA Nexus attacker-interest assessment

`compose-storage-work --attacker-interest-output /absolute/new/assessment.json`
adds a read-only economic-interest assessment to the existing complete native
composition pipeline. It groups captured records and typed references into
balance, LP, farming, rewards, debt, staking, bridge and issuance review families.
It neither mutates state nor authorizes an economic disposition.

Add the option to the full existing composition invocation with its verified
capture, mapping/proofs, complete inventory and semantic/bridge/staking reviews.
A bare `inventory` invocation is insufficient at this checkpoint: incompatible
BridgeMultisig payloads require the source-proven native adapter resolutions
constructed by normal composition.

The capture must pass full state-root/header verification and executed-Wasm
metadata authentication. Composition adds the economic plan's exact
AccountId32/H160 registry to the watch, performs the complete native traversal
with all adapter resolutions, then constructs this assessment from its private
in-memory certificate. Serialized inventory or success reports cannot replace
that certificate. Complete reviewed legacy key preimages, opaque retention and
semantic dispositions remain required; diagnostic caller-defined codecs cannot
clear the certificate checks.

The output file is create-new and fsynced before economic acceptance. A positive
interest still makes the subsequent production coverage check fail. Therefore
`compositionSucceeded=false` remains fixed in this diagnostic even when the
surrounding command later succeeds. Retain the actual command exit/logs and
compiled work separately; a diagnostic file surviving a failed invocation is
never successful composition evidence. Omitting the flag preserves existing
composition behavior and output bytes.

The fixed incident AccountId32 and H160 remain mandatory. Extra identities must
be explicitly supplied and are never inferred from a pool, reserve, technical
account, address resemblance, transaction counterparty or Ethereum asset.

A zero-reference result means only that no directly typed reference was observed
for the exact registry. It does not establish beneficial or derived-account
ownership, absent claims in opaque history, external-chain assets, TS/SS burn
entitlements, a complete attacker registry, or the safety of a refund. A positive
reference reports its exact storage key/value hash and typed field path; a pool
account appearing in the first LP map key is not automatically the owner of the
provider shares in the second key. Unknown named stores and child tries remain
in `other_storage`; the family catalog is not an acceptance allowlist.

The assessment retains `attackerAssetsDebtAndIssuanceVerified=false` and
`productionReady=false` for every result. Existing production composition still
rejects positive attacker references until the exact ownership/backing adapter
exists. It also rejects economic operations combined with a zero-reference
certificate. This command does not change those decisions.

Positive positions require the following coupled accounting before execution:

- LP withdrawal must preserve exact pool rounding, fees, supply, indexes and
  lockers, then burn the withdrawn underlying assets. Farm snapshots and lazy
  rewards refer to the same underlying interest and cannot be counted twice.
- Reserve-backed rewards need exact reserve attribution, aggregate totals and
  account reference accounting. `Rewards` and `VestedRewards` normally transfer
  existing reserves; an arbitrary reward scalar is not proof of an unissued
  claim.
- Debt requires settlement or enforced non-withdrawable custody of enough
  collateral. Preserving the bytes of a collateral row alone does not implement
  custody or protect aggregate lender liabilities after a proposed burn.
- Refunds require counterpart finality and complete redeemable-authorization
  evidence. Cancelling a request or seeing no local signatures cannot justify a
  refund or burning pooled bridge backing. The incoming exclusion path's
  preservation of owned bridge custody is a distinct operation.
- Every actual issuance change must join exact checkpoint supply, authenticated
  incoming minting, approved burns and the resulting state. Captured issuance
  row counts do not prove that equation.

Retained reports at the agreed checkpoint found zero direct typed references
for the two currently supplied identities across 1,208,536 top entries and no
child tries. That observation does not remove the qualifications above and must
be refreshed using the final pinned verifier and inputs. Any future additional
identity or ownership evidence requires a fresh complete assessment.

For an owned root validation lane, use a generous deadline and its existing warm
target:

```sh
SKIP_WASM_BUILD=1 cargo test --locked -p incident-recovery --lib interest_assessment_
SKIP_WASM_BUILD=1 cargo clippy --locked -p incident-recovery --all-targets -- -D warnings
```

The staged candidate has six native unit regressions covering zero observations,
positive balances, shared LP key roles, H160 and uncatalogued hits, altered
certificates/registries, and claimed positions inconsistent with observed
absence. Compilation and actual command execution are separate validation steps.
