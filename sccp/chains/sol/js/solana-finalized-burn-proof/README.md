# `@sccp-sol/solana-finalized-burn-proof`

Browser-facing helpers for the Solana -> SORA finalized burn proof flow.

What this package does:

- encodes the canonical SCCP `BurnPayloadV1`
- derives the canonical `messageId = keccak256("sccp:burn:v1" || payload)`
- derives the canonical Solana burn-record PDA from `(program_id, messageId)`
- fetches the burn-record account from Solana RPC at `finalized`
- decodes the Solana burn-record account data that the proof must bind to
- validates that the burn-record account, owner, PDA, and payload all bind to the requested `messageId`
- extracts the public inputs consumed by SORA's `SolanaFinalizedBurnProofV1`
- encodes the full versioned SCALE proof envelope expected by SORA:
  - `public_inputs`
  - `burn_proof`
  - `vote_proofs`

What this package does not do:

- verify a STARK/FRI proof locally
- fetch or synthesize the finalized bank/account witness on its own
- fetch or synthesize validator vote messages on its own

The intended flow is:

1. The UI calls `fetchFinalizedBurnProofInputs(...)` to derive the burn-record PDA, fetch the finalized account, and canonicalize the public inputs.
2. A validator-side witness exporter or proof service supplies:
   - `burnProof`
   - one or more `voteProofs`
3. The UI calls `buildFinalizedBurnProof(...)` and submits `proofBytes` to SORA.

The reference exporter/builder pair in this repo is:

- [`tools/solana-account-proof-geyser/`](../../tools/solana-account-proof-geyser)
- [`tools/solana-finalized-burn-witness/`](../../tools/solana-finalized-burn-witness)
