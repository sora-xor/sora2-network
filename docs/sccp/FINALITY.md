# SCCP Finality (Inbound Proofs To SORA)

This document defines **what it means for a burn on a remote chain to be final**, as required to mint on **SORA** via SCCP.

SCCP is designed to be **fail-closed**: if a proof cannot be verified on-chain, minting on SORA must not happen.

## Common Requirements (All Chains)

An inbound mint on SORA (from `source_domain != SORA`) must only succeed if the submitted proof shows:

1. **Authenticity**: the burn was executed by the SCCP router/program/contract for the asset on the source chain.
2. **Inclusion**: the burn message id (`messageId`) is included in the source chain’s canonical state (event log, receipt, account/storage state, or program state), for a specific block/slot/sequence.
3. **Finality**: that block/slot/sequence is finalized under the source chain’s finality rules (defined below).
4. **Non-replay**: the `messageId` has not been processed before on SORA.
5. **Incident Controls**: SORA governance has not paused inbound from that `source_domain`, and has not invalidated that specific `messageId`.

Items 4 and 5 are enforced by `pallet-sccp` state. Items 1-3 are source-chain-specific and must be verified by an on-chain **light client** (or equivalent on-chain verifier) inside the SORA runtime.

## Definitions

- **messageId**: `keccak256(b"sccp:burn:v1" || BurnPayloadV1_scale_bytes)` (canonical across chains).
- **Burn record on source chain**: the chain-specific representation of a burn that binds `messageId` to:
  - `asset_id` (SORA asset id, 32 bytes)
  - `amount`
  - `recipient32`
  - `nonce`
  - `source_domain` / `dest_domain`
- **Finality**: a chain-specific condition under which the probability of reorg that removes the burn is *negligible* (probabilistic finality) or *cryptographically prevented* (BFT finality).

## Ethereum (DOMAIN_ETH = 1)

### Finality Definition

Ethereum is PoS with **cryptographic finality** (Casper FFG).

A burn is final for SORA **only if** it is included in an **execution-layer block** whose corresponding **consensus-layer header is finalized** (i.e., part of the finalized checkpoint).

### Verifier Requirements (On SORA)

To verify an ETH burn on-chain, a SORA verifier must:

- Maintain an Ethereum consensus light client capable of verifying:
  - finalized beacon headers, via sync committee signatures and updates
  - the mapping from finalized beacon header to execution payload header (block hash)
- Verify the burn’s inclusion by proving either:
  - a receipt/log proof for an emitted `Burn(messageId, ...)` event in the SCCP router contract, or
  - a storage proof that a router state mapping contains `messageId -> BurnRecord`, at that finalized block.

### Practical Note

If a first implementation uses **confirmation depth** (e.g., `N` blocks) instead of beacon finality, it is *not* Ethereum-finality and should be treated as a temporary, weaker mode.

## BSC (DOMAIN_BSC = 2)

### Finality Definition

BSC uses validator-based consensus. Depending on fork/version, it may provide either:

- a **stronger finality signal** (e.g., fast-finality attestations), or
- only **probabilistic finality** (reorgs are possible).

For SCCP inbound-to-SORA, BSC finality should be defined as:

- burn is final when included in a block that is at least `k` blocks deep from the current head of the verified canonical chain,
  - and the chain of headers is valid under BSC’s header signature rules and validator-set rules.

`k` should be configured conservatively and treated as a security parameter.

### Verifier Requirements (On SORA)

SORA must maintain a BSC header-chain verifier that:

- verifies each header’s validator signature(s) per BSC consensus rules,
- tracks/updates validator sets as required by the chain,
- exposes a finalized block hash/number per the chosen `k`-deep rule,
- verifies burn inclusion via receipt/log proof or storage proof at that block.

Practical details for modern BSC headers:

- the Parlia seal hash is **not** simply `keccak256(rlp(header_without_sig))`; it includes `chainId` and must include
  the Cancun-era optional header fields when present (see `core/types.EncodeSigHeader` in BSC).
- the verifier must account for `turn_length` (sprint length) in both:
  - the expected `difficulty` (`2` for in-turn, `1` otherwise), and
  - the recent-signer rule window length.

## Solana (DOMAIN_SOL = 3)

### Finality Definition

Solana provides a notion of **finalized** slots (supermajority confirmation).

A burn is final for SORA only if included in a **finalized slot** under Solana consensus.

### Fail-Closed Status

There is no SCCP attester fallback anymore. If the Solana finalized-burn verifier is unavailable,
`pallet-sccp` rejects inbound proofs until the proof backend is live.

### Verifier Requirements (On SORA)

SORA must maintain a Solana light client that can:

- verify finalized slots (vote signatures + stake weights, per Solana finality rules),
- verify finalized bank/account inclusion for the canonical SCCP burn-record PDA keyed by `messageId`, and
- verify that the proven burn-record account state binds to the canonical `messageId`.

The runtime proof envelope for this path is `SolanaFinalizedBurnProofV1`.
Its public inputs bind:

- `message_id`
- `finalized_slot`
- configured Solana SCCP router program id
- burn-record PDA
- burn-record owner
- burn-record account data hash

Because Solana finality depends on stake distribution and vote verification, this is substantially more complex than EVM receipt proofs alone.

## TON (DOMAIN_TON = 4)

### Finality Definition

TON finality is derived from the **masterchain**:

A shardchain burn is final for SORA only if:

1. the burn is included in a shardchain block, and
2. that shardchain block (or its hash) is referenced by a masterchain block, and
3. that masterchain block is final under TON validator signatures and masterchain rules.

### Verifier Requirements (On SORA)

SORA consumes a native `TonBurnProofV1` bundle anchored to a governance-pinned trusted
TON checkpoint `(mc_seqno, mc_block_hash)`. The proof bundle is expected to carry:

- masterchain progression data from the trusted checkpoint to a target finalized masterchain block,
- shard linkage proving the shard state is finalized under that masterchain block,
- account-state proof for the configured SCCP jetton master, and
- dictionary inclusion data for `burns[messageId]`.

`pallet-sccp` binds that proof to the configured TON identity:

- `RemoteToken[asset_id, DOMAIN_TON]` = expected jetton master account id,
- `DomainEndpoint[DOMAIN_TON]` = expected jetton master code hash.

## TRON (DOMAIN_TRON = 5)

### Finality Definition

TRON is DPoS with a protocol notion of **irreversible (solidified)** blocks.

For SCCP inbound-to-SORA, TRON finality is defined as:

- burn is final when included in a **solidified** block under TRON consensus, i.e. a block that has been
  "approved" by more than `70%` of the active witnesses by producing subsequent blocks (TRON mainnet: `19/27`).

### Verifier Requirements (On SORA)

SORA must maintain a TRON header verifier that:

- verifies witness signatures on header `raw_data` and binds the recovered signer to `witness_address`,
- tracks a finalized (solidified) header pointer using the >`70%` distinct-witness rule,
- verifies SCCP burn inclusion via an on-chain verifiable state proof against that finalized header's execution root
  (e.g. `accountStateRoot` + EVM MPT storage proof for `burns[messageId]` in the SCCP router contract).

## Summary Table

| Domain | Chain | Finality Type | Definition Used By SCCP-to-SORA |
|---:|---|---|---|
| 1 | Ethereum | Cryptographic | CL finalized checkpoint (Casper FFG) |
| 2 | BSC | Probabilistic | `k`-deep confirmed header chain + valid validator signatures |
| 3 | Solana | Cryptographic-ish (supermajority) | Solana finalized slot |
| 4 | TON | Cryptographic (BFT) | Masterchain finality + shard proof |
| 5 | TRON | DPoS solidification | Solidified block (>70% distinct witnesses) + verified header signatures |

## Implementation Status In This Repo

- `pallet-sccp` is designed to **fail closed** for inbound-to-SORA proof verification unless an on-chain verifier can validate the proof.
- Current implementation includes:
  - **ETH beacon-mode integration hook** in `pallet-sccp` via `EthFinalizedBurnProofVerifier`:
  - when an on-chain verifier can validate a finalized ETH burn proof, SCCP uses it in `EthBeaconLightClient` mode
  - if the verifier is unavailable, ETH remains fail-closed (`InboundFinalityUnavailable`)
  - **SOL light-client integration hook** in `pallet-sccp` via `SolanaFinalizedBurnProofVerifier`:
  - when an on-chain verifier is available, SCCP uses it for SOL burn verification in `SolanaLightClient` mode
  - if the verifier is unavailable, SOL remains fail-closed (`InboundFinalityUnavailable`)
  - **TON native proof path** in `pallet-sccp`:
  - governance pins a trusted TON checkpoint with `set_ton_trusted_checkpoint(mc_seqno, mc_block_hash)`
  - users submit SCALE-encoded `TonBurnProofV1` bytes in `TonLightClient` mode
  - if no checkpoint is configured, TON remains fail-closed (`InboundFinalityUnavailable`)
  - **BSC on-chain header verifier** (k-deep) feeding a finalized EVM `state_root` for MPT proofs.
  - **TRON on-chain header verifier** (solidified-block rule) feeding a finalized execution root (`accountStateRoot`) for MPT proofs.
- Finality definition is now explicit on-chain per source domain via:
  - `sccp.set_inbound_finality_mode(domain_id, mode)`
  - mode checks are enforced by `mint_from_proof` and `attest_burn` before proof verification
- Default mode mapping in `pallet-sccp`:
  - ETH (`1`): `EthBeaconLightClient` (hooked, fail-closed in production runtime until wired)
  - BSC (`2`): `BscLightClient`
  - SOL (`3`): `SolanaLightClient` (hooked, fail-closed in production runtime until wired)
  - TON (`4`): `TonLightClient` (native checkpointed proof consumption; fail-closed until a trusted checkpoint is configured)
  - TRON (`5`): `TronLightClient`
- Governance incident controls already exist in `pallet-sccp`:
  - pause inbound per `source_domain`
  - invalidate specific inbound `messageId`
  - pause outbound burns to a specific `dest_domain`

## Outbound Governance Commitments (SORA -> Destinations)

Destination lifecycle synchronization uses the same finalized SORA digest model as outbound mint proofs:

- `activate_token` commits `TokenAddPayloadV1` with prefix `sccp:token:add:v1`
- `pause_token` commits `TokenControlPayloadV1` with prefix `sccp:token:pause:v1`
- `resume_token` commits `TokenControlPayloadV1` with prefix `sccp:token:resume:v1`

These commitments are per-destination-domain and consume a dedicated SCCP governance nonce on SORA.
Destination verifiers prove inclusion of one canonical governance `messageId` in a finalized SORA digest, then
apply the lifecycle transition locally. Initial verifier bootstrap remains a deployment/bootstrap concern, not a
proof-driven destination governance flow.

## Proof Tooling

Operator-facing proof generation commands are documented in:

- `docs/sccp/PROOF_TOOLING.md`

Inbound-to-SORA proof generation (source chains -> SORA) is documented in:

- `docs/sccp/INBOUND_TOOLING.md`
