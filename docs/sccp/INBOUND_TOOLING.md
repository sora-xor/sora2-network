# SCCP Proof Tooling (Source Chains -> SORA)

This document describes how to generate the proof artifacts required for **minting on SORA** from a burn on another chain.

It focuses on the modes currently implemented in `pallet-sccp`:

- EVM storage proofs (`eth_getProof`, EIP-1186) for Ethereum/BSC/TRON
- governance anchor mode finality (execution `state_root` provided by governance)
- BSC on-chain light client finality (Parlia header verifier + `k`-deep finality)
- TRON on-chain light client finality (witness header verifier + “solidified” finality)

SCCP is **fail-closed**: if the required finality/verifier state is not available on-chain, SORA will reject the proof.

## 1) Preconditions On SORA (Governance)

For a given `source_domain` (ETH/BSC/TRON):

1. Configure the remote SCCP router endpoint: `sccp.set_domain_endpoint(source_domain, endpoint_id)`.
2. Ensure the asset is registered in SCCP and has a remote token id set for that `source_domain`:
   `sccp.add_token(asset_id)`, then `sccp.set_remote_token(asset_id, source_domain, remote_token_id)`, then
   `sccp.activate_token(asset_id)` (requires all SCCP core remote domains ETH/BSC/SOL/TON/TRON are configured;
   `RequiredDomains` must include all core domains and can only add extra requirements).
3. Choose finality mode (per domain): `sccp.set_inbound_finality_mode(source_domain, mode)`.

Supported modes today:

- ETH (`1`): default `EthBeaconLightClient` (hooked, fail-closed in production runtime until wired). Temporary overrides: `EvmAnchor`, `AttesterQuorum`.
- BSC (`2`): default `BscLightClient`. Optional fallback mode: `BscLightClientOrAnchor` or explicit `EvmAnchor`.
- SOL (`3`): default `SolanaLightClient` (hooked, fail-closed in production runtime until wired). Temporary override: `AttesterQuorum`.
- TON (`4`): default `TonLightClient` (hooked, fail-closed in production runtime until wired). Temporary override: `AttesterQuorum`.
- TRON (`5`): default `TronLightClient`. Temporary overrides: `EvmAnchor`, `AttesterQuorum`.

### EVM Anchor Mode (ETH/BSC/TRON)

1. Enable anchor mode: `sccp.set_evm_anchor_mode_enabled(source_domain, true)`.
2. Set a finalized anchor: `sccp.set_evm_inbound_anchor(source_domain, block_number, block_hash, state_root)`.

SORA will verify inbound burns by checking an on-chain verifiable **MPT storage proof** against that anchored `state_root`.

### BSC Light Client Mode

1. Bootstrap the on-chain header verifier once:
   `sccp.init_bsc_light_client(checkpoint_header_rlp, validators, epoch_length, confirmation_depth, chain_id, turn_length)`.
2. Keep it up to date permissionlessly: `sccp.submit_bsc_header(header_rlp)` repeatedly.

Once `BscFinalized` is available on-chain, SORA can verify `eth_getProof` storage proofs at the finalized header’s `state_root`.

Helper tooling:

```bash
bridge-relayer --evm-url <BSC_RPC_URL> sccp evm header-rlp \
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
bridge-relayer sccp tron header \
  --tron-api-url <TRON_API_BASE_URL> \
  --block-number <BLOCK_NUMBER>
```

This returns `raw_data_hex` and `witness_signature_hex` bytes ready for SORA TRON light-client extrinsics.

## 2) Burn On The Source Chain (User)

Burn on the source chain SCCP router/program/contract targeting SORA:

- The burn must have `dest_domain = SORA (0)`.
- The burn produces a canonical `payload` and `messageId`.

On EVM routers (`sccp-eth`, `sccp-bsc`, `sccp-tron`), this is `SccpRouter.burnToDomain(...)` and the event `SccpBurned(messageId, ..., payload)` includes both.

## 3) Generate The Proof (User / Relayer)

### EVM Chains: `eth_getProof` (EIP-1186)

SORA verifies the existence of the burn record by proving that, at the anchored/finalized block:

- the SCCP router account exists (account proof)
- the router storage contains a non-zero value at the slot for `burns[messageId].sender` (storage proof)

Use the `bridge-relayer` CLI:

```bash
bridge-relayer --evm-url <EVM_RPC_URL> sccp evm burn-proof-to-sora \
  --router <ROUTER_ADDRESS_0x...> \
  --message-id <MESSAGE_ID_0x...> \
  --block-number <BLOCK_NUMBER>
```

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

- Solana (`3`) and TON (`4`) is **fail-closed** in the production runtime until real on-chain light clients are wired
  into the `pallet-sccp` verifier hooks.

As a practical CCTP-style fallback, governance can enable `InboundFinalityMode::AttesterQuorum` for any domain by:

1. configuring the attester set + threshold with `sccp.set_inbound_attesters(domain, attesters, threshold)`
2. switching the finality mode with `sccp.set_inbound_finality_mode(domain, AttesterQuorum)`

### AttesterQuorum Proof Format (Bytes)

SORA verifies a threshold of ECDSA signatures from a configured on-chain attester set over:

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
