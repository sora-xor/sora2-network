# SCCP TRON Light Client (Inbound TRON -> SORA)

This document describes the **on-chain TRON header verifier** integrated into `pallet-sccp` and how it is used to accept **TRON -> SORA** burn proofs in `InboundFinalityMode::TronLightClient`.

The implementation is intentionally **fail-closed**: malformed headers, invalid signatures, forks, or missing prerequisites cause verification to fail.

## What Is Verified On-Chain

For each imported TRON header, SORA verifies:

- `BlockHeader.raw_data` protobuf decoding (subset used by SCCP):
  - `parentHash` (field 3, bytes32)
  - `number` (field 7, varint)
  - `witness_address` (field 9, 21 bytes)
  - `accountStateRoot` (field 11, bytes32) as the execution-state root used for EVM MPT proofs
- witness ECDSA signature over `sha256(raw_data)` (TRON rule)
  - signature must be **non-malleable** (`r != 0`, `s != 0`, and `s <= secp256k1n/2`)
- witness address binding:
  - `witness_address == address_prefix || eth_address20(recovered_pubkey)`
- recovered witness must be present in the governance-configured witness set
- linear chain extension only (no forks):
  - submitted header must be exactly `head.number + 1` and point to `head.hash`

## Finality Definition Used By SCCP

SCCP models TRON "irreversible / solidified" finality as:

- a block is considered finalized once it is followed by blocks produced by more than `70%` of the active witnesses (TRON mainnet: `19/27`),
- where the signer set in the finality window is **distinct**.

On-chain this is tracked in:

- `sccp.tron_head()` (latest imported header)
- `sccp.tron_finalized()` (latest "solidified" header)

## Enabling TRON Trustless Inbound To SORA

1. Configure SCCP domain endpoint on SORA (router contract address on TRON):
   - `sccp.set_domain_endpoint(SCCP_DOMAIN_TRON, <20-byte-router-address>)`

2. Initialize the TRON light client (governance):
   - `sccp.init_tron_light_client(checkpoint_raw_data, checkpoint_witness_signature, witnesses, address_prefix)`

Inputs:
- `checkpoint_raw_data`: **protobuf bytes** of `BlockHeader.raw_data`
- `checkpoint_witness_signature`: 65 bytes `r(32)||s(32)||v(1)` for `sha256(raw_data)`
- `witnesses`: sorted unique list of witness `H160` addresses (eth-style `keccak256(pubkey)[12..]`)
- `address_prefix`: TRON address prefix byte (mainnet commonly `0x41`)

Governance must pick a checkpoint block that is already known to be irreversible on TRON.

3. Switch inbound finality mode for TRON (governance):
   - `sccp.set_inbound_finality_mode(SCCP_DOMAIN_TRON, TronLightClient)`

4. Keep the header chain progressing (permissionless):
   - `sccp.submit_tron_header(raw_data, witness_signature)`

Operator helper:

```bash
bash ./sccp/tools/sccp-proof.sh tron header \
  --rpc <TRON_API_BASE_URL> \
  --block-number <TRON_BLOCK_NUMBER>
```

Use `raw_data_hex` and `witness_signature_hex` from the output as extrinsic inputs.

As headers are imported, `tron_finalized` will advance when the >70% distinct-witness condition is met.

If the witness set changes on TRON, governance must update it on SORA:

- `sccp.set_tron_witnesses(witnesses)`

## Burn Proofs (Storage Proofs) Under TRON Finality

Once `TronLightClient` mode is active, `mint_from_proof` / `attest_burn` for `source_domain = TRON` verifies:

- the proof is an Ethereum-style account/storage MPT proof (`EvmBurnProofV1`)
- against the `accountStateRoot` of the **finalized** TRON header (`sccp.tron_finalized().state_root`)
- proving that the SCCP router contract storage contains a non-zero record for `burns[messageId].sender`

This is the same EVM account/storage proof format used for BSC finalized-state verification, but the root is obtained **trustlessly** from the TRON light client.
