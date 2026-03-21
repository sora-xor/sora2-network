# sccp-sol

SORA Cross-Chain Protocol (SCCP) code for Solana.

Current contents:
- A small Rust `no_std` crate implementing SCCP `BurnPayloadV1` SCALE encoding and `messageId`
  computation (`keccak256(b"sccp:burn:v1" || payload)`).

- A Solana program crate under `program/`:
  - config PDA (outbound nonce, immutable verifier program id, legacy unused governor field kept for wire compatibility)
  - per-asset token registry PDA (`sora_asset_id -> SPL mint`)
  - `Burn` burns SPL tokens via CPI and stores an on-chain burn record PDA keyed by `messageId`
  - `MintFromProof` is implemented but **fail-closed** until the verifier program is bound once during bootstrap
  - local admin mutation paths are disabled; proof flow remains permissionless after bootstrap
  - domain hardening: burn/mint paths reject unsupported domain IDs

## Build / Test

```bash
cargo test
```

Formal-assisted checks:

```bash
./scripts/test_formal_assisted.sh
./scripts/test_ci_formal.sh
```

Deploy script smoke checks:

```bash
./scripts/test_deploy_scripts.sh
./scripts/check_repo_hygiene.sh
```

Program tests:

```bash
cd program
cargo test
```

Fuzz tests (bounded):

```bash
./scripts/test_fuzz_nightly.sh
./scripts/test_ci_fuzz.sh
./scripts/test_ci_all.sh
```

`scripts/run_fuzz_bounded.sh` auto-installs `cargo-fuzz` on first run by default.
It also self-provisions the selected Rust toolchain profile if `cargo` is missing.
To require manual installation instead, omit `--auto-install` when invoking `scripts/run_fuzz_bounded.sh`.

Current verifier coverage includes:
- positive import-root + mint proof verification flow
- duplicate validator key rejection
- insufficient-signature threshold rejection
- high-`s` ECDSA signature rejection (malleability hardening)
- unsupported/loopback source-domain rejection in verifier burn-proof path

## Non-SORA -> Solana (Via SORA Attestation)

The Solana `MintFromProof` path is not limited to burns that originated on SORA.

If SORA verifies a burn that originated on another chain (e.g., `ETH -> SOL`) and commits the burn `messageId`
into its auxiliary digest (via the SORA runtime extrinsic `sccp.attest_burn`), users can mint on Solana by
submitting:

- `source_domain = <burn origin domain>`
- `payload = SCALE(BurnPayloadV1)`
- `proof = SORA BEEFY+MMR proof that the digest commits `messageId``

## Proofs To SORA (SOL As Source Chain)

Inbound proofs from Solana to SORA are defined on SORA as:

- default mode: `SolanaLightClient` for `DOMAIN_SOL`
- semantics: burn must be included in a Solana finalized slot
- proof target: the canonical SCCP burn-record PDA/account state keyed by `messageId`
- proof envelope: SCALE(`SolanaFinalizedBurnProofV1`) carrying:
  - versioned public inputs
  - `burn_proof` for the finalized burn-record PDA account inclusion
  - `vote_proofs` showing validator votes over a later `SlotHashes` witness that commits the burn slot's bank hash
- current runtime status: the SORA verifier now re-derives the canonical burn-record binding, validates the account/bank proof shape, verifies raw Solana vote signatures, and enforces the configured authority quorum
- fail-closed status: there is no SCCP attester fallback; if the finalized-burn verifier backend is unavailable, inbound Solana proofs are rejected

Canonical browser-side helpers for this flow now live under [`js/solana-finalized-burn-proof/`](./js/solana-finalized-burn-proof).
They encode `BurnPayloadV1`, derive `messageId`, decode the Solana burn-record account, extract the public inputs SORA expects,
and wrap a witness bundle into `SolanaFinalizedBurnProofV1`.

The off-chain witness builder now lives under [`tools/solana-finalized-burn-witness/`](./tools/solana-finalized-burn-witness).
It consumes validator-exported Geyser account-proof updates plus Solana RPC vote blocks, assembles the canonical `burn_proof`/`vote_proofs`,
and emits the exact SCALE proof bytes SORA expects.

The reference validator-side exporter now lives under [`tools/solana-account-proof-geyser/`](./tools/solana-account-proof-geyser).
It runs inside a Solana validator as a Geyser plugin, watches the configured burn-record PDA set plus `SlotHashes`,
and streams Borsh `Update` values directly to the witness builder over TCP.

So this repo supports trustless SORA -> Solana mint verification today, and a concrete Solana -> SORA finalized-burn proof path built around canonical burn-record inclusion plus vote-quorum verification.

## Proof Inputs

Use the in-repo SCCP proof tooling described in [docs/sccp/PROOF_TOOLING.md](/Users/mtakemiya/dev/sora2-network/docs/sccp/PROOF_TOOLING.md) to generate:

- verifier bootstrap inputs for `Initialize`
- finalized-root import inputs for `SubmitSignatureCommitment`
- mint-proof JSON containing `mmr_proof`, `mmr_leaf`, and `digest_scale`

This command already outputs the verifier-ready `SoraBurnProofV1` Borsh bytes:
- `borsh_proof_hex`
- `borsh_proof_base64`

The helper script is still available for re-encoding historical or externally generated proof JSON:
- `python3 scripts/encode_sora_burn_proof.py --input ./mint-proof.json --format both`

## SORA Config Notes

On SORA (runtime pallet `sccp`), for Solana:
- `domain_endpoint` for `SCCP_DOMAIN_SOL` is 32 bytes: the SCCP Solana program id.
- `remote_token_id` for a given `asset_id` is 32 bytes: the SPL mint pubkey for that asset.

SORA token activation also requires `set_domain_endpoint(SCCP_DOMAIN_SOL, <program_id_bytes>)`.
