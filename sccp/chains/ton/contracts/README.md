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
  - minting is fail-closed until a verifier is configured and the token is Active
- Lifecycle governance:
  - the canonical jetton master is predeployed and starts `Paused`
  - `SccpAddTokenFromVerifier`, `SccpPauseTokenFromVerifier`, and `SccpResumeTokenFromVerifier`
    are private verifier-only messages
  - the verifier derives the canonical governance `messageId`, verifies finalized SORA digest inclusion,
    then forwards the lifecycle message to the master
  - governance replay protection is keyed by `messageId` on the master

Security posture:
- the canonical master is immutable after deployment except for the one-time verifier bootstrap path
- verifier bootstrap and one-time verifier binding are governor-gated deployment/bootstrap steps
- add/pause/resume lifecycle changes are proof-driven from SORA and accepted only from the configured verifier
- both mint and burn are gated on `Active`; `Paused` blocks user burns and verifier mints
- all other local admin mutation paths are disabled; proof flow remains fail-closed thereafter

Entrypoints:
- `contracts/sccp-jetton-master.tolk`
- `contracts/sccp-jetton-wallet.tolk`
- `contracts/sccp-sora-verifier.tolk`
