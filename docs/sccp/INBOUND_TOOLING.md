# SCCP Proof Tooling (Source Chains -> SORA)

This document describes how to generate the proof artifacts required for **minting on SORA** from a burn on another chain.

It focuses on the modes currently implemented in `pallet-sccp`:

- EVM storage proofs (`eth_getProof`, EIP-1186) for Ethereum/BSC/TRON
- BSC on-chain light client finality (Parlia header verifier + `k`-deep finality)
- TON checkpointed proof consumption (`TonBurnProofV1`)
- TRON on-chain light client finality (witness header verifier + “solidified” finality)

SCCP is **fail-closed**: if the required finality/verifier state is not available on-chain, SORA will reject the proof.

## 1) Preconditions On SORA (Governance)

For a given `source_domain`:

1. Configure the remote SCCP router endpoint: `sccp.set_domain_endpoint(source_domain, endpoint_id)`.
2. Ensure the asset is registered in SCCP and has a remote token id set for that `source_domain`:
   `sccp.add_token(asset_id)`, then `sccp.set_remote_token(asset_id, source_domain, remote_token_id)`, then
   `sccp.activate_token(asset_id)` (requires all SCCP core remote domains ETH/BSC/SOL/TON/TRON are configured;
   `RequiredDomains` must include all core domains and can only add extra requirements).
3. Choose finality mode (per domain): `sccp.set_inbound_finality_mode(source_domain, mode)`.

Supported modes today:

- ETH (`1`): default `EthBeaconLightClient` (hooked, fail-closed in production runtime until wired). Additional proof-backed mode: `EthZkProof` (native runtime STARK/FRI verifier, no trusted setup).
- BSC (`2`): default `BscLightClient`.
- SOL (`3`): default `SolanaLightClient` (hooked, fail-closed in production runtime until wired).
- TON (`4`): default `TonLightClient`. Governance bootstraps a trusted checkpoint with `set_ton_trusted_checkpoint`.
- TRON (`5`): default `TronLightClient`.

### BSC Light Client Mode

1. Bootstrap the on-chain header verifier once:
   `sccp.init_bsc_light_client(checkpoint_header_rlp, validators, epoch_length, confirmation_depth, chain_id, turn_length)`.
2. Keep it up to date permissionlessly: `sccp.submit_bsc_header(header_rlp)` repeatedly.

Once `BscFinalized` is available on-chain, SORA can verify `eth_getProof` storage proofs at the finalized header’s `state_root`.

Helper tooling:

```bash
cd sccp/chains/bsc
npm run build-bsc-header-rlp -- \
  --rpc-url <BSC_RPC_URL> \
  --block-number <BLOCK_NUMBER> \
  --bsc-epoch-length <EPOCH_LENGTH>
```

### TRON Light Client Mode

1. Bootstrap once with a known solidified checkpoint:
   `sccp.init_tron_light_client(checkpoint_raw_data, checkpoint_witness_signature, witnesses, address_prefix)`.
2. Keep importing headers permissionlessly: `sccp.submit_tron_header(raw_data, witness_signature)` repeatedly.

Once `TronFinalized` is available, SORA can verify EVM-style storage proofs at the solidified header’s `accountStateRoot`.

Helper tooling:

```bash
bash ./sccp/tools/sccp-proof.sh tron header \
  --rpc <TRON_API_BASE_URL> \
  --block-number <BLOCK_NUMBER>
```

This returns `raw_data_hex` and `witness_signature_hex` bytes ready for SORA TRON light-client extrinsics.

### TON Light Client Mode

1. Bootstrap the TON trust root on SORA:
   `sccp.set_ton_trusted_checkpoint(mc_seqno, mc_block_hash)`.
2. Keep token identity aligned:
   - `sccp.set_remote_token(asset_id, DOMAIN_TON, jetton_master_account_id_32)`
   - `sccp.set_domain_endpoint(DOMAIN_TON, jetton_master_code_hash_32)`
3. Build a SCALE `TonBurnProofV1` bundle in `sccp/chains/ton`:

```bash
cd sccp/chains/ton
npm run encode-ton-burn-proof-to-sora -- \
  --jetton-master <ton_addr> \
  --sora-asset-id 0x<32-byte> \
  --recipient 0x<32-byte> \
  --amount <u128> \
  --nonce <u64> \
  --checkpoint-seqno <u32> --checkpoint-hash 0x<32-byte> \
  --target-seqno <u32> --target-hash 0x<32-byte> \
  --masterchain-proof <0xhex|base64|@file> \
  --shard-proof <0xhex|base64|@file> \
  --account-proof <0xhex|base64|@file> \
  --burns-dict-proof <0xhex|base64|@file>
```

The encoder prints `proof_scale_hex` / `proof_scale_base64` for direct submission to
`sccp.mint_from_proof`.

## 2) Burn On The Source Chain (User)

Burn on the source chain SCCP router/program/contract targeting SORA:

- The burn must have `dest_domain = SORA (0)`.
- The burn produces a canonical `payload` and `messageId`.

On EVM routers (`sccp-eth`, `sccp-bsc`, `sccp-tron`), this is `SccpRouter.burnToDomain(...)` and the event `SccpBurned(messageId, ..., payload)` includes both.

## 3) Generate The Proof (User / Any Untrusted Submitter)

### EVM Chains: `eth_getProof` (EIP-1186)

SORA verifies the existence of the burn record by proving that, at the anchored/finalized block:

- the SCCP router account exists (account proof)
- the router storage contains a non-zero value at the slot for `burns[messageId].sender` (storage proof)

For BSC, use the canonical `sccp-bsc` tooling that matches the on-chain router event and storage layout:

```bash
cd sccp/chains/bsc
npm run extract-burn-proof-inputs -- \
  --receipt-file <BURN_RECEIPT_JSON> \
  --router <ROUTER_ADDRESS_0x...>

npm run build-burn-proof-to-sora -- \
  --rpc-url <BSC_RPC_URL> \
  --router <ROUTER_ADDRESS_0x...> \
  --payload <PAYLOAD_0x...> \
  --block <BLOCK_NUMBER>
```

This emits:

- `proof_scale_hex`: SCALE `EvmBurnProofV1` bytes ready for `sccp.mint_from_proof` / `sccp.attest_burn`
- `burns_slot_base`: raw storage slot requested from `eth_getProof`
- `storage_trie_key`: trie key that SORA derives internally for MPT verification
- `block.hash` and `block.state_root`: the exact execution root the proof was built against

For other EVM-like domains, use the local MCP helper in `misc/sccp-mcp` to build the exact SCALE bytes expected by
`pallet-sccp` from `eth_getProof`:

```json
{
  "name": "evm_sccp_build_burn_proof",
  "arguments": {
    "network": "eth_mainnet",
    "payload": {
      "version": 1,
      "source_domain": 1,
      "dest_domain": 0,
      "nonce": 7,
      "sora_asset_id": "0x<32-byte>",
      "amount": "123",
      "recipient": "0x<32-byte>"
    },
    "router": "0x<20-byte>",
    "block": "finalized"
  }
}
```

The MCP result includes:

- `proof_scale_hex`: SCALE `EvmBurnProofV1` bytes ready for `sccp.mint_from_proof` / `sccp.attest_burn`
- `block.hash` and `block.state_root`: the exact execution root the proof was built against
- `burns_slot_base`: raw storage slot requested from `eth_getProof`
- `storage_trie_key`: trie key computed from that slot for MPT verification
- `suggested_sora_call`: a prefilled SCCP call shape when `payload` is provided

Output fields:

- `proof_scale_hex`: SCALE bytes expected by SORA `pallet-sccp` (pass as `proof` argument)
- `block_hash`: must match the block hash that SORA expects for the selected finality mode
- anchor mode: it must match the governance-configured anchor block hash
- BSC light client mode: it must match the current `BscFinalized.hash` stored on SORA
- TRON light client mode: it must match the current `TronFinalized.hash` stored on SORA

Important: if SORA’s finalized pointer advances, a proof generated at an older block hash will be rejected.

## 4) Submit To SORA

### Mint on SORA (Source -> SORA)

Call:

- `sccp.mint_from_proof(source_domain, payload, proof_scale_bytes)`

SORA will:

- enforce incident controls (paused domains, invalidated messages)
- enforce token state (active/grace-period)
- verify the proof on-chain according to `inbound_finality_mode(source_domain)`
- mint the SORA asset to `payload.recipient` (converted to SORA `AccountId`)
- mark `messageId` as processed (replay protection)

### Attest on SORA (Source -> SORA -> Destination Hub Flow)

If the original burn targeted a non-SORA destination (`dest_domain != SORA`), call:

- `sccp.attest_burn(source_domain, payload, proof_scale_bytes)`

This verifies the burn on-chain, then commits `messageId` into SORA’s auxiliary digest so the destination chain can mint
by verifying SORA finality (BEEFY+MMR light client).

## 5) Current Limitations

Inbound-to-SORA proof verification for:

- ETH (`1`) now has a native `EthZkProof` verifier in the runtime for SCALE-encoded
  `EthZkFinalizedBurnProofV1` envelopes. Beacon-mode `EthBeaconLightClient` remains fail-closed
  until a finalized ETH state provider is wired.
- Solana (`3`) now has a fixed `SolanaFinalizedBurnProofV1` envelope and runtime verifier hook binding
  `messageId` plus the configured Solana router id, but remains **fail-closed** until a real on-chain
  Solana/STARK verifier backend is wired into that hook.
- TON (`4`) remains fail-closed until governance configures `set_ton_trusted_checkpoint(...)`.

Deprecated fallback modes (`EvmAnchor`, `BscLightClientOrAnchor`, `AttesterQuorum`) now fail closed in `pallet-sccp`
and should not be used for new SCCP flows.

`attestHash = keccak256("sccp:attest:v1" || messageId)`

Important:
- this is a raw digest signature (no `"\x19Ethereum Signed Message:\n32"` prefix).
- signatures must be 65 bytes (`r || s || v`); `v` may be `0/1` or `27/28`.
- duplicate signers in one proof are rejected (fail-closed).

`proof` bytes passed to `sccp.mint_from_proof` / `sccp.attest_burn` are:

- `version: u8 = 1`
- `signatures: SCALE(Vec<[u8;65]>)`

So the final bytes are: `0x01 || SCALE(signatures_vec)`.

The on-chain, trustless part already exists in the opposite direction:

- destination chains verify SORA commitments using BEEFY+MMR light clients (`sccp-eth`, `sccp-bsc`, `sccp-tron`, `sccp-sol`, `sccp-ton`).
