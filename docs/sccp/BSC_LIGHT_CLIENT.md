# SCCP BSC Light Client (Inbound To SORA)

This document describes the **on-chain BSC header verifier** embedded in `pallet-sccp` and how it is used to mint on SORA
from burns on BSC.

## Purpose

For `source_domain = BSC`, `pallet-sccp` can verify inbound burns **without governance anchors** by:

1. Maintaining an on-chain BSC header chain verifier (Clique/Parlia style signatures).
2. Exposing a finalized EVM `state_root` using a `k`-deep rule (`confirmation_depth`).
3. Verifying an Ethereum MPT storage proof that `burns[messageId]` exists in the SCCP router contract state.

## Governance Bootstrap

Governance must initialize the verifier once:

- `sccp.init_bsc_light_client(checkpoint_header_rlp, validators, epoch_length, confirmation_depth, chain_id, turn_length)`

Where:

- `checkpoint_header_rlp`: RLP-encoded BSC header (full header, including extraData signature)
- header signatures are verified with EIP-2 style malleability hardening: non-zero `r/s` and canonical low-`s`
- `validators`: list of validator addresses (`H160`) used for in-turn checks (the pallet sorts them ascending internally)
  - duplicate entries are rejected (fail-closed)
- `epoch_length`: validator-list epoch length (used to detect epoch blocks)
- `confirmation_depth`: `k` for the `k`-deep finality rule
- `chain_id`: BSC EVM chain id used for Parlia seal-hash verification (`56` on BSC mainnet)
- `turn_length`: Parlia turn length (a.k.a. sprint length), used for difficulty + recent-signer checks (e.g., `16` on BSC after Maxwell)

### Helper Tooling (Off-chain)

To fetch the canonical block header RLP bytes from an EVM RPC (and optionally extract the epoch validator list from `extraData`),
use the sibling `bridge-relayer` CLI:

```bash
bridge-relayer --evm-url <BSC_RPC_URL> sccp evm header-rlp \
  --block-number <CHECKPOINT_BLOCK_NUMBER> \
  --bsc-epoch-length <EPOCH_LENGTH>
```

This command fail-fast checks that `keccak256(rlp(header))` matches the node-provided block hash before outputting `header_rlp_hex`.

After initialization, anyone can import headers:

- `sccp.submit_bsc_header(header_rlp)`

If BSC rotates validators, the pallet can **apply validator-set updates automatically** from epoch headers
at the Parlia activation point. Governance can still update the configured set as an emergency override:

- `sccp.set_bsc_validators(validators)`
  - duplicate entries are rejected (fail-closed)

## Finalized Header Used By SCCP

The verifier tracks:

- `bsc_head`: latest imported header
- `bsc_finalized`: header considered finalized under the chosen `confirmation_depth`

Inbound SCCP mints from BSC require that `bsc_finalized` exists.

## Mint Proof Format

SORA uses the same proof type as other EVM-like domains:

- `EvmBurnProofV1 { anchor_block_hash, account_proof, storage_proof }`

For BSC, `anchor_block_hash` **must equal** the on-chain `bsc_finalized.hash`.

The proof is verified against `bsc_finalized.state_root`.

## Security Notes / Current Limitations

- The verifier currently accepts **linear extension only** (it does not implement fork-choice). If a fork occurs and a
  non-canonical branch is imported, the verifier can get stuck (fail-closed for SCCP minting).
- Validator-set updates are applied only when an epoch header is available in the imported header history and the
  Parlia activation point is reached. Governance should still monitor the verifier and be prepared to intervene
  (pause inbound from BSC, re-initialize, etc.) in case of consensus-rule mismatches or unexpected fork behavior.
- This verifier is intended to be upgraded over time to fully match BSC consensus/finality details (including trustless
  validator-set rotation and fast-finality attestations) as needed.
