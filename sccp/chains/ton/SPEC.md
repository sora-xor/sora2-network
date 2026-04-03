# SCCP Message Format (v1)

Migration note: payload encoding remains relevant, but verifier/finality sections in this package
still reflect the pre-Nexus SORA BEEFY/MMR hub model. The active repo tooling consumes finalized
Nexus bundles through `scripts/build_master_call_from_nexus_bundle.mjs`.

This repo follows the SCCP message format pinned by the SORA runtime pallet `sccp`.

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

Encoding: Substrate SCALE encoding for fixed-width primitives (no compact encoding is used here).

Total encoded length: `97` bytes.

## messageId

On all chains, the canonical message id is:

`messageId = keccak256(b"sccp:burn:v1" || payload_scale_bytes)`

Where `payload_scale_bytes` is the SCALE encoding of `BurnPayloadV1`.

## Recipient Encoding

`recipient` is always 32 bytes. Each chain interprets it differently:

- EVM (ETH/BSC/TRON): `address` right-aligned (last 20 bytes), i.e. `address(uint160(uint256(recipient)))`.
  - Canonical encoding is enforced on-chain: the top 12 bytes must be zero and the address must be non-zero.
- Solana: 32-byte ed25519 public key
- TON: 32-byte address/public-key representation (project-specific; must be consistent across contracts and off-chain tooling)
- TON (this repo): `recipient` is the 256-bit account id (32 bytes) of a standard address in **workchain 0**.
  - The on-chain recipient address is interpreted as `address.fromWorkchainAndHash(0, recipient32)`.

## Remote Token IDs (stored on SORA)

When SORA governance adds an SCCP token, it stores each remote representation id:

- EVM (ETH/BSC/TRON): 20 bytes (contract address)
- Solana: 32 bytes (mint)
- TON: 32 bytes (jetton master account id; workchain is assumed `0` for the address encoding)

## BEEFY Validator Merkle Proofs (SORA -> TON)

The TON verifier contract (`contracts/sccp-sora-verifier.tolk`) is a BEEFY+MMR light client.

When importing a BEEFY commitment, signers must prove membership in the current validator set
using the `nextAuthoritySetRoot` merkle root from the SORA MMR leaf.

Root construction matches Substrate `binary_merkle_tree` (no sorting):

- Leaves: `leaf = keccak256(bytes20(validator_eth_address))`
- Internal nodes: `parent = keccak256(left || right)`
- Odd leaf promotion: if a layer has an odd number of nodes, the last node is promoted unchanged

Each signature includes:

- `position`: validator leaf index (0-based) in the set order used by the chain
- the merkle sibling list for that index (one sibling per tree level where a sibling exists)
- ECDSA signature validity rules: `r != 0`, `s != 0`, and `s <= secp256k1n / 2` (reject high-`s` malleability)
- verifier implementation rejects duplicate signer addresses in one commitment proof (fail-closed)

## Proofs To SORA Finality (TON Source)

For inbound TON -> SORA verification, SORA runtime defines:

- `InboundFinalityMode::TonLightClient` (default for `DOMAIN_TON`)
- proof bytes: SCALE-encoded `TonBurnProofV1`
- trust root: governance-pinned TON trusted checkpoint `(mc_seqno, mc_block_hash)`
- proof subject: the jetton master burn record stored at `burns[messageId]`

`TonBurnProofV1` carries:

- trusted checkpoint seqno + hash
- target masterchain seqno + hash
- jetton master account id (32 bytes)
- jetton master code hash (32 bytes)
- canonical burn record fields:
  - `dest_domain`
  - `recipient32`
  - `jetton_amount`
  - `nonce`
- raw proof blobs for:
  - masterchain progression
  - shard linkage
  - account state
  - `burns` dictionary inclusion

The helper CLI in this repo is:

- `npm run encode-ton-burn-proof-to-sora -- ...`
