# SCCP Hub Transfers

SCCP no longer uses SORA2 as the hub. The hub now lives in `../iroha` as **Sora Nexus mainnet**,
and this runtime acts as a spoke that consumes finalized Nexus proof bundles.

## Current Flow

1. A user burns on a source spoke.
2. Nexus verifies the spoke burn proof and publishes a canonical SCCP hub commitment.
3. The destination spoke calls `mint_from_proof(proof_bundle)` with a finalized Nexus burn bundle.

For governance-controlled token lifecycle actions, Nexus verifies a **Sora Parliament**
enactment certificate first, then publishes a finalized SCCP governance bundle.
Spokes consume those bundles via:

- `add_token_from_proof`
- `pause_token_from_proof`
- `resume_token_from_proof`

There is no local `attest_burn` step on SORA2 anymore, and there is no local SCCP governance path
for token lifecycle changes.

## Proof Source

Spokes trust finalized Nexus SCCP bundles, not local SORA2 governance or SORA BEEFY/MMR commitments.
The proof bundles are served by Torii in `../iroha`:

- `GET /v1/sccp/proofs/burn/{message_id}`
- `GET /v1/sccp/proofs/governance/{message_id}`

Each bundle carries:

- the canonical SCCP payload,
- the Nexus SCCP hub commitment,
- the SCCP Merkle inclusion proof,
- a Nexus finality proof,
- and for governance, the parliament certificate bytes and hash.

## Spoke Responsibilities

This pallet now does only spoke-side work:

- permissionless outbound `burn`
- permissionless inbound `mint_from_proof`
- permissionless inbound governance application from Nexus proofs
- replay protection for inbound proof consumption

This pallet does not provide:

- `attest_burn`
- local SCCP token add/pause/resume administration
- local SCCP incident-control governance
- SORA2 hub commitments for downstream chains

## Governance Source

Governance proofs must originate from **Sora Parliament on Nexus**. A spoke should only accept
governance state transitions that arrive as finalized Nexus SCCP governance bundles.
