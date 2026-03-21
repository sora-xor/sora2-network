# Contracts

SCCP on TON is implemented as Jetton contracts (Tolk), one per bridged SORA asset.

High-level responsibilities:

- Outbound: `SccpBurnToDomain` message to the user's Jetton wallet
  - burns jettons (supply decreases)
  - stores an on-chain burn record in the Jetton master keyed by `messageId`
- Inbound: `SccpMintFromVerifier` message to the Jetton master
  - `mint_from_proof` is implemented as a verifier-triggered mint:
    - a separate on-chain verifier/light client must validate the burn on the source chain
    - the configured verifier contract then calls the master to mint
  - minting is fail-closed until a verifier is configured

Security posture:
- minting must be fail-closed until a verifier is deployed and configured
- local admin mutation paths are disabled; verifier bootstrap is one-time and proof flow remains fail-closed thereafter
 
Entrypoints:
- `contracts/sccp-jetton-master.tolk`
- `contracts/sccp-jetton-wallet.tolk`
- `contracts/sccp-sora-verifier.tolk`
