# SCCP EVM Inbound (Temporary Anchor Mode)

This document describes the **temporary** inbound-to-SORA verification mode for EVM-like domains where SORA either:

- does not yet have a fully trustless finality light client (currently: **ETH**), or
- needs an **emergency fallback** finality source during incidents (optionally: **BSC/TRON**).

This mode is intended as a **stopgap**. It is **not** a full trustless light client and should be replaced by
chain-specific finality light clients as described in `docs/sccp/FINALITY.md`.

## Overview

To mint on SORA from an EVM chain, the SORA runtime verifies that a burn message id (`messageId`) is present in the
SCCP router contract storage at a governance-provided finalized EVM state root.

SORA governance provides (via on-chain extrinsics):

- `domain_id` (ETH/BSC/TRON)
- `block_hash`
- `state_root` (execution state root for that block)

Users provide:

- an Ethereum MPT account proof for the SCCP router account (to obtain the contract `storageRoot`)
- an Ethereum MPT storage proof for the `burns[messageId]` mapping entry

## Governance Configuration

1. Set router address for the source domain (SCCP endpoint):
   - `sccp.set_domain_endpoint(domain_id, router_address_20bytes)`
2. Enable anchor mode for the domain (opt-in safety switch):
   - `sccp.set_evm_anchor_mode_enabled(domain_id, true)`
3. Set the anchor:
   - `sccp.set_evm_inbound_anchor(domain_id, block_number, block_hash, state_root)`

If no anchor is set, inbound EVM mints are rejected (`EvmInboundAnchorMissing`).

## Proof Format (`EvmBurnProofV1`)

`mint_from_proof` expects `proof: Vec<u8>` to be the SCALE encoding of:

- `anchor_block_hash: H256`
- `account_proof: Vec<Vec<u8>>` (RLP-encoded MPT nodes)
- `storage_proof: Vec<Vec<u8>>` (RLP-encoded MPT nodes)

The proof is accepted only if:

- `anchor_block_hash` equals the on-chain stored anchor `block_hash` for that `domain_id`
- the account proof is valid against `state_root`
- the storage proof is valid against the router account’s `storageRoot`
- the proven storage value is non-zero

## Storage Slot Proven

SORA verifies that the SCCP router storage slot for `burns[messageId].sender` is non-zero.

In the canonical `SccpRouter` Solidity implementation, the mapping slot is:

- `SCCP_EVM_BURNS_MAPPING_SLOT = 4`

The base slot is computed as:

- `slot_base = keccak256(messageId || u256_be(4))`

The storage trie key used by Ethereum is:

- `storage_key = keccak256(slot_base)`

SORA computes this internally from the submitted `payload` and does not require it in the proof.

## Generating Proofs Off-chain (Typical RPC)

Most EVM JSON-RPC providers expose `eth_getProof` which returns:

- `accountProof`: RLP-encoded MPT nodes for the account path
- `storageProof[0].proof`: RLP-encoded MPT nodes for a specific storage slot

For SCCP, the requested storage slot is `slot_base` (not `storage_key`).
