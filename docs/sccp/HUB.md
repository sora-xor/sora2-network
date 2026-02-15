# SCCP Hub Transfers (Non-SORA -> Non-SORA)

SCCP supports direct transfers between two non-SORA chains by using **SORA as a trustless verification hub**:

1. User burns on `source_domain` targeting `dest_domain != SORA`.
2. User submits the source-chain proof to SORA via `sccp.attest_burn(...)`.
3. SORA verifies the burn (fail-closed) and commits the burn `messageId` into its **auxiliary digest**.
4. User mints on `dest_domain` by proving that SORA has committed `messageId` into a **BEEFY+MMR finalized** digest.

This is the SCCP analogue of "burn -> attest -> mint" where the attestation is **on-chain and decentralized**
(BEEFY signatures + MMR inclusion), and the only privileged operations are SCCP configuration and incident controls.

## SORA Extrinsic: `attest_burn`

`attest_burn(origin, source_domain, payload, proof)`:

- verifies `payload` is a valid `BurnPayloadV1` (canonical encoding)
- applies incident controls:
  - inbound paused for `source_domain` blocks attestations
  - outbound paused for `payload.dest_domain` blocks attestations
  - invalidated `messageId` blocks attestations
- requires token configuration exists on SORA for both source and destination:
  - `RemoteToken[asset_id, source_domain]` exists
  - `RemoteToken[asset_id, dest_domain]` exists
- verifies the burn proof using the chain-specific verifier for `source_domain`
  - finality requirements are enforced via `inbound_finality_mode(source_domain)` in `pallet-sccp`
  - verification is **fail-closed** (no verifier => no attestation)
- records `AttestedOutbound[messageId] = true` to prevent duplicate commitments
- commits `messageId` to the auxiliary digest as:
  - `AuxiliaryDigestItem::Commitment(GenericNetworkId::EVMLegacy('SCCP'), messageId)`

Destination chains can then mint by verifying that commitment in SORA finalized state.

## Proof Tooling

Operational proof-generation commands are documented in:

- `docs/sccp/PROOF_TOOLING.md`

Implementation lives in the sibling `bridge-relayer` repository under CLI group `sccp`.

## Destination-Chain Minting

On the destination chain, the SCCP router/program/jetton master verifies:

- `payload` decodes to a valid `BurnPayloadV1`
- `payload.dest_domain == local_domain`
- the burn `messageId` is included in a finalized SORA digest commitment, proven via an on-chain **SORA BEEFY+MMR light client**

Important: destination-chain verifiers only attest that **SORA committed** the `messageId`. They do not need to
re-verify the original source chain, because SORA already did that in `attest_burn`.

## Incident Response

SORA governance can contain incidents without upgrading contracts:

- pause inbound from a source domain: `sccp.set_inbound_domain_paused(source_domain, true)`
- pause outbound to a destination domain: `sccp.set_outbound_domain_paused(dest_domain, true)`
- block a specific inbound burn forever: `sccp.invalidate_inbound_message(source_domain, message_id)`

All checks are enforced on-chain; when a flag is set, operations fail-closed.
