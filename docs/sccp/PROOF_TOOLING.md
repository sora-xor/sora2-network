# SCCP Proof Tooling (SORA -> Destination Chains)

This document defines how to generate proof artifacts used by SCCP destination-chain verifiers.

For inbound-to-SORA proof generation (source chain -> SORA), see:

- `docs/sccp/INBOUND_TOOLING.md`

## Source Of Truth

SCCP proof tooling now lives in this repository:

- `sccp/tools/sccp-proof.sh` for local SCCP proof-helper dispatch
- `sccp/chains/sol/scripts/encode_sora_burn_proof.py` for Solana verifier-ready Borsh proof bytes
- `sccp/chains/ton/scripts/encode_sora_proof_cell.mjs` for TON verifier-ready proof cells
- `sccp/chains/eth`, `sccp/chains/bsc`, `sccp/chains/tron` for destination verifier contracts and chain-native helper scripts

The old external SCCP proof CLI is deprecated for this repo.

## Command Overview

### 1) Export validator sets

Purpose:
- collect `latest_beefy_block`
- collect current/next BEEFY validator sets (`id`, `len`, `root`)
- initialize destination verifier governance state

Chain-specific outputs:
- `sccp sol init`: Borsh instruction bytes for Solana verifier `Initialize`
- `sccp ton init`: TON message body BOC for `SccpVerifierInitialize`

### 2) Import finalized MMR root

Input:
- SORA block containing BEEFY justification

Output:
- commitment payload (`mmr_root`, `block_number`, `validator_set_id`)
- validator signatures + Merkle membership proofs
- MMR leaf and Substrate MMR proof (`leaf_index`, `leaf_count`, `items`)

Chain-specific outputs:
- `sccp evm import-root`: JSON fields for Solidity call params
- `sccp sol import-root`: Borsh instruction bytes for Solana `SubmitSignatureCommitment`
- `sccp ton import-root`: verifier-ready TON cells/BOCs:
  - validator proof cell
  - latest leaf proof cell
  - submit message body

### 3) Build mint proof

Input:
- `burn_block` where SORA committed SCCP `messageId` into auxiliary digest
- `beefy_block` with finalized MMR root context
- `message_id`

Output:
- `digest_scale` (SCALE bytes of auxiliary digest at `burn_block`)
- `mmr_leaf` + `mmr_proof`
- optional ABI-packed bytes (`--abi`) for Solidity verifier calls

Chain-specific outputs:
- Solana: Borsh proof bytes for `SoraBurnProofV1`
- TON: proof cell BOC for verifier mint messages
- EVM chains: verifier-ready inputs consumed by their in-repo contracts and scripts

## Safety Properties

The flow is fail-closed:

- destination verifier checks imported MMR root is finalized by BEEFY signatures
- destination verifier checks MMR inclusion of leaf + digest hash binding
- destination verifier checks digest includes exactly one SCCP commitment for the submitted `messageId`

If any condition fails, mint must fail.

## Runtime Requirements On SORA

Trustless SCCP proof generation depends on:

- BEEFY justifications available on-chain
- MMR proof RPC (`mmr_generateProof`)
- `leaf_provider::LatestDigest` available at historical block hashes

In this repository, this corresponds to running with the trustless bridge/MMR stack enabled (`runtime` `wip` feature path).
