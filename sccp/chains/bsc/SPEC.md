# SCCP Message Format (v1)

Migration note: payload encoding remains relevant, but verifier/finality sections in this package
still reflect the pre-Nexus SORA BEEFY/MMR hub model. The active repo tooling consumes finalized
Nexus bundles through `scripts/build_router_call_from_nexus_bundle.mjs`.

This repo follows SCCP message conventions pinned by the SORA runtime pallet `sccp`.

## Domains

- `0`: SORA
- `1`: Ethereum
- `2`: BSC
- `3`: Solana
- `4`: TON
- `5`: TRON

## BurnPayloadV1

Fields (in order):
1. `version: u8` (must be `1`)
2. `source_domain: u32` (little-endian)
3. `dest_domain: u32` (little-endian)
4. `nonce: u64` (little-endian)
5. `sora_asset_id: [u8; 32]`
6. `amount: u128` (little-endian)
7. `recipient: [u8; 32]`

Total encoded length: `97` bytes.

## TokenAddPayloadV1

Fields (in order):
1. `version: u8` (must be `1`)
2. `target_domain: u32` (little-endian)
3. `nonce: u64` (little-endian)
4. `sora_asset_id: [u8; 32]`
5. `decimals: u8`
6. `name: [u8; 32]` (zero-padded printable ASCII)
7. `symbol: [u8; 32]` (zero-padded printable ASCII)

Total encoded length: `110` bytes.

## TokenPausePayloadV1 / TokenResumePayloadV1

Fields (in order):
1. `version: u8` (must be `1`)
2. `target_domain: u32` (little-endian)
3. `nonce: u64` (little-endian)
4. `sora_asset_id: [u8; 32]`

Total encoded length: `45` bytes.

## Message IDs

Canonical IDs are:
- Burn: `keccak256("sccp:burn:v1" || burn_payload_bytes)`
- Token add: `keccak256("sccp:token:add:v1" || token_add_payload_bytes)`
- Token pause: `keccak256("sccp:token:pause:v1" || token_pause_payload_bytes)`
- Token resume: `keccak256("sccp:token:resume:v1" || token_resume_payload_bytes)`

## Recipient Encoding

`recipient` is always 32 bytes:
- EVM (ETH/BSC/TRON): right-aligned 20-byte address.
- EVM canonical constraints on router:
  - high 12 bytes must be zero,
  - address must be non-zero.
- Solana / TON: project-specific 32-byte representation.

## BEEFY Validator Merkle Proofs (SORA -> EVM)

When importing a BEEFY commitment, signers prove membership in validator set root:
- leaf: `keccak256(bytes20(validator_eth_address))`
- parent: `keccak256(left || right)`
- odd leaf promotion unchanged

Verifier checks:
- threshold signatures (`>=2/3`),
- valid positions and membership proofs,
- duplicate signer/public-key rejection,
- ECDSA low-`s` and non-zero `r,s`.

## Unified Proof Envelope

Burn and governance action verifications consume:

`abi.encode(uint64 leafIndex, uint64 leafCount, bytes32[] items, MmrLeaf leaf, bytes digestScale)`

Where:
- `leaf.digestHash == keccak256(digestScale)`
- `digestScale` includes exactly one SCCP commitment hash (`messageId`)
- MMR root derived from `(leaf, proof items)` must be known/imported.

## BSC Burn Event Proof Target

Canonical outbound `BSC -> SORA` burn proof target:

`SccpBurned(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed sender, uint128 amount, uint32 destDomain, bytes32 recipient, uint64 nonce, bytes payload)`

Proof consumers should enforce:
- log address equals the configured BSC router address,
- `topic0 == keccak256("SccpBurned(bytes32,bytes32,address,uint128,uint32,bytes32,uint64,bytes)")`,
- `topic1 == messageId`,
- `messageId == keccak256("sccp:burn:v1" || payload)`,
- decoded `payload` fields match the non-indexed event fields,
- decoded `payload.sora_asset_id` matches indexed `soraAssetId`,
- decoded `payload.source_domain == 2`.

## Router Lifecycle Rules

- Token is created only via proven `TokenAddPayloadV1`.
- Token pause/resume only via proven `TokenPausePayloadV1` / `TokenResumePayloadV1`.
- Paused token blocks both outbound burn and inbound mint.
- Governance action replay protection uses message-id uniqueness.
- Burn replay protection remains per burn message-id.
