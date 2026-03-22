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
Governance payloads are first-class here as well: destination codecs and verifiers share the same canonical
message-id model for burn minting and token lifecycle proofs.

## Command Overview

### 1) Export validator sets

Purpose:
- collect `latest_beefy_block`
- collect current/next BEEFY validator sets (`id`, `len`, `root`)
- build the one-time governor-authorized bootstrap payload for each destination verifier

Note: these bootstrap payloads are local destination-chain initialization messages, not finalized SORA governance proofs.

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

### 4) Build governance proof

Input:
- SORA block where governance committed one lifecycle `messageId`
- `message_id` for one of:
  - `keccak256("sccp:token:add:v1" || TokenAddPayloadV1_scale_bytes)`
  - `keccak256("sccp:token:pause:v1" || TokenControlPayloadV1_scale_bytes)`
  - `keccak256("sccp:token:resume:v1" || TokenControlPayloadV1_scale_bytes)`

Output:
- the same finalized MMR leaf/proof material used for mint proofs
- digest bytes proving that the canonical governance `messageId` was committed by SORA
- destination-native verifier artifacts for add/pause/resume flows

Chain-specific outputs:
- EVM chains: verifier call params consumed by `addTokenFromProof`, `pauseTokenFromProof`, `resumeTokenFromProof`
- Solana: Borsh instruction bytes plus governance payload/message-id helpers in `sccp/chains/sol`
- TON: verifier proof cell BOCs plus governance payload/message-id helpers in `sccp/chains/ton`

## Safety Properties

The flow is fail-closed:

- destination verifier checks imported MMR root is finalized by BEEFY signatures
- destination verifier checks MMR inclusion of leaf + digest hash binding
- destination verifier checks digest includes exactly one SCCP commitment for the submitted `messageId`

If any condition fails, mint or lifecycle mutation must fail.

## Runtime Requirements On SORA

Trustless SCCP proof generation depends on:

- BEEFY justifications available on-chain
- MMR proof RPC (`mmr_generateProof`)
- `leaf_provider::LatestDigest` available at historical block hashes

In this repository, this corresponds to running with the trustless bridge/MMR stack enabled (`runtime` `wip` feature path).
