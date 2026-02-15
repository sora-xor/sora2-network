# SCCP (SORA Cross-Chain Protocol)

SCCP is a burn/mint cross-chain protocol intended to be **fully on-chain**:

- burns create on-chain burn records + deterministic `messageId`
- mints require an on-chain verifiable proof that the burn `messageId` is finalized
- SORA governance only manages configuration and incident response; verification is intended to be light-client based
- token activation on SORA enforces deployed remote representations + endpoints on all SCCP core target domains (ETH/BSC/SOL/TON/TRON)
- SCCP token registration is exclusive with legacy bridge routes: `add_token` rejects assets already on legacy bridges (EVM/TON), including queued legacy EVM `add_asset` requests
- inbound finality modes for ETH/SOL/TON are wired through pluggable on-chain verifier hooks:
  `EthFinalizedStateProvider`, `SolanaFinalizedBurnProofVerifier`, `TonFinalizedBurnProofVerifier`

## Docs In This Repo (SORA)

- `docs/sccp/FINALITY.md`: inbound-to-SORA finality definitions per source chain
- `docs/sccp/INBOUND_TOOLING.md`: how to generate and submit inbound proofs to SORA (EVM anchor + BSC/TRON light clients)
- `docs/sccp/HUB.md`: non-SORA -> non-SORA transfers via SORA on-chain attestation
- `docs/sccp/PROOF_TOOLING.md`: SORA -> destination proof generation (BEEFY+MMR) for destination verifiers
- `docs/sccp/EVM_ANCHOR_MODE.md`: governance-anchored EVM mode details
- `docs/sccp/BSC_LIGHT_CLIENT.md`: BSC header verifier details (inbound-to-SORA)
- `docs/sccp/TRON_LIGHT_CLIENT.md`: TRON header verifier details (inbound-to-SORA)

## Code In This Repo (SORA)

- `pallets/sccp/`: SCCP pallet (token registry, burns, mints, attestation, incident controls, BSC/TRON inbound verifiers)

## Sibling Repos (Destination Chains)

These repos implement SCCP routers/programs and **SORA BEEFY+MMR light-client verifiers** for minting on each chain:

- `../sccp-eth`
- `../sccp-bsc`
- `../sccp-tron`
- `../sccp-sol`
- `../sccp-ton`

## Tooling

Proof generation is implemented in:

- `../bridge-relayer` (`bridge-relayer sccp ...`)
