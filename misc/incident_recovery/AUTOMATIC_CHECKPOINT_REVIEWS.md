# SORA Nexus automatic checkpoint reviews

These offline generators prepare deterministic inputs for the native recovery
compiler. They do not provide human approvals, ownership signatures, external
finality evidence, refunds, or a release-ready policy. Use the provisional CSV
only for testing; regenerate the entire graph when the final CSV changes.

Start with the complete root-verified checkpoint capture, executing-Wasm metadata
witness, the original source-derived inventory options and an empty semantic plan
(`{}`). The inventory options contain source-pinned legacy codecs, exact hashed
key preimages and immutable opaque retention proofs. Run the current native
`verify-trusted-mapping` against those exact inputs. Fixture-derived inventory
dispositions are rejected by this preparation path.

```sh
node misc/incident_recovery/prepare_checkpoint_reviews.mjs \
  --capture /absolute/checkpoint \
  --verification /absolute/fresh-capture-verification.json \
  --metadata-witness /absolute/executed-metadata-witness.json \
  --trusted-mapping /absolute/fresh-trusted-mapping.json \
  --csv /absolute/mapping.csv \
  --operators /absolute/derived-checkpoint-operators.scale \
  --base-inventory /absolute/source-inventory-options.json \
  --base-semantic /absolute/empty-semantic.json \
  --output /absolute/new-general-reviews
```

The generator streams every captured storage key and validates each page's
checksum, header, count, ordering, cursor and terminal marker. Manifest range
hints are ignored. The resulting observed page ranges select the complete rows
for governance, elections, staking, referrals, democracy and offence reviews.
Large compressed pages are hashed incrementally; the existing capture protocol's
256 MiB decompression bound and bounded row parser remain enforced.

The output inventory preserves all exact key preimages and immutable proofs.
It derives a new disposition for every observed prefix. An explicit source
allowlist permits financial ownership moves with unchanged quantities and
reference counts. Other prefixes retain their checkpoint bytes unless a native
adapter resolves them; this retention cannot waive a mapped mutable identity.
Session, Permissions and pending Iroha multisig state therefore still require
their native adapters. Staking payout defaults to the replacement owner, mapped
nominators are chilled, and generated utility descendants come from the native
trusted mapping report. Unresolved control or custody cases remain blockers.

Next prepare local bridge and scheduler reviews, using the retained historical
BridgeMultisig preimage evidence directory:

```sh
node misc/incident_recovery/prepare_checkpoint_bridge_reviews.mjs \
  --capture /absolute/checkpoint \
  --verification /absolute/fresh-capture-verification.json \
  --metadata-witness /absolute/executed-metadata-witness.json \
  --trusted-mapping /absolute/fresh-trusted-mapping.json \
  --prepared-reviews /absolute/new-general-reviews \
  --historical-evidence /absolute/verified-bridge-preimages \
  --output /absolute/new-bridge-reviews
```

This emits complete HASHI prefix commitments, local signing cancellation and
BridgeProxy history reviews. Historical request bytes, signatures and replay
records are preserved. It does not invent exceptions for ambiguous historical
owners. Nonterminal outgoing liabilities are quarantined without a refund.
The existing BridgeMultisig cancellation generator verifies each recovered
preimage and its authenticated source before producing bookkeeping changes.
The scheduler generator records exact custody and opaque call hashes without
executing historical calls. Their selective scans are admitted only after the
original manifest range hints match the independently observed page ranges.

Use the bridge output's `semantic.json` and the general output's `inventory.json`
for composition, together with their individual review files and the scheduler
`review.json`. Re-run native trusted mapping and full composition on those final
inputs; the earlier admission report is bound to the original baseline options.
Keep every input, source pin, generation report and execution log in the bound
release evidence. Generated data and successful generator exit codes do not
replace native accounting, incoming-transfer authentication, finality/history,
external reconciliation or exact-artifact rehearsal gates.

To select the next public source captures, inventory the checkpoint's bridge
registries without contacting any network:

```sh
node misc/incident_recovery/inventory_checkpoint_bridge_sources.mjs \
  --capture /absolute/checkpoint \
  --verification /absolute/fresh-capture-verification.json \
  --metadata-witness /absolute/executed-metadata-witness.json \
  --prepared-reviews /absolute/new-general-reviews \
  --output /absolute/new-source-registry-inventory
```

Its report includes typed public contract/channel configuration, asset routes,
nonce rows, data-signer and inbound-verifier peer sets, and counts of retained
requests by exact protocol network identity. It also records absent registries.
An internal HASHI `u32`/`EVMLegacy` ID is never converted into an external EVM
chain ID. Generic `EVM(H256)` IDs are separate and retain the production app's
`wip` qualification. Substrate network labels and TON channel/app addresses do
not establish endpoint identity, external finality or current bridge activity.
The report is an input to choosing authenticated source captures, not evidence
that incoming-transfer reconciliation is complete.
