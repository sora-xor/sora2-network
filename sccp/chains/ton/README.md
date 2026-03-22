# sccp-ton

SORA Cross-Chain Protocol (SCCP) components for TON (Jetton-based).

This repo contains:
- The canonical SCCP message format (`SPEC.md`)
- A Jetton master + wallet implementation in **Tolk** with SCCP extensions
  (burn records + verifier-gated minting + proof-driven lifecycle state)
- A trustless SORA->TON verifier contract (`contracts/sccp-sora-verifier.tolk`) that implements a BEEFY+MMR light client on TON

Compiler/tooling:
- `@ton/tolk-js` (Tolk v1.2.0)
- Node.js 22 (`.nvmrc`, `package.json#engines`)

## Notes

TON does not share EVM's log/event model; SCCP "proof of burn" is expected to be based on
state (burn record cells) and/or transaction proofs, verified by a dedicated on-chain
verifier/light client on the destination chain.

For SORA->TON, this repo uses the same approach as the EVM/Solana SCCP verifiers:
- finalized SORA MMR roots are imported permissionlessly by verifying BEEFY commitments (validator signatures + merkle membership proofs)
- SCCP burn proofs are verified via MMR inclusion + auxiliary digest commitment, then forwarded to the Jetton master for minting
- SCCP governance proofs are verified the same way for `addTokenFromProof`, `pauseTokenFromProof`, and `resumeTokenFromProof`
- verifier hardening: ECDSA signatures must have non-zero `r/s` and canonical low-`s` (`s <= secp256k1n/2`)
- verifier hardening: duplicate validator signer addresses in a commitment proof are rejected (fail-closed)
- router hardening: TON jetton wallet/master reject unsupported SCCP domain ids in burn/mint flows
- verifier bootstrap requires the configured governor, and the master accepts a one-time governor-pinned verifier binding before proof flow opens
- the canonical jetton master is predeployed, starts paused, and is activated by a SORA governance proof after SORA whitelists its canonical identity
- the master accepts lifecycle changes only from its configured verifier; there is no steady-state local governor/operator mutation path
- all other local admin mutation paths are disabled

## Non-SORA -> TON (Via SORA Attestation)

TON minting is not restricted to burns that originated on SORA.

If SORA verifies a burn that originated on another chain (e.g., `ETH -> TON`) and commits the burn `messageId`
into its auxiliary digest (via the SORA runtime extrinsic `sccp.attest_burn`), this repo can mint on TON by sending
the verifier message:

- `SccpVerifierMintFromSoraProofV2` (opcode `0x1a9b2c7e`)

This message includes an explicit `sourceDomain` and computes `messageId` as:

`keccak256("sccp:burn:v1" || BurnPayloadV1(sourceDomain, DOMAIN_TON, ...))`

## Proofs To SORA (TON As Source Chain)

The updated SORA2 pallet consumes a repo-defined SCALE payload directly:

- finality mode: `TonLightClient`
- proof type: `TonBurnProofV1`
- governance bootstrap: `sccp.set_ton_trusted_checkpoint(mc_seqno, mc_block_hash)`
- per-token binding on SORA:
  - `remote_token_id` = TON jetton master account id (`address.hash`, 32 bytes)
  - `domain_endpoint` = TON jetton master code hash (32 bytes)

`TonBurnProofV1` carries:

- the trusted checkpoint `(mc_seqno, mc_block_hash)` it is anchored to
- the target finalized masterchain block `(seqno, block_hash)`
- the jetton master account id and code hash expected by SORA
- the canonical burn record fields proven from `burns[messageId]`
- proof bytes for the masterchain, shard, account-state, and `burns` dictionary legs

The masterchain and shard legs are themselves SCALE-encoded section payloads:

- `masterchain_proof` = `TonMasterchainProofSectionV1(version, checkpoint_block_boc, checkpoint_state_extra_proof_boc, target_block_proof_boc, target_state_extra_proof_boc)`
- `shard_proof` = `TonShardProofSectionV1(version, shard_block_boc, shard_state_accounts_proof_boc)`

This repo now owns the proof-byte encoder for that format:

```bash
npm run encode-ton-burn-proof-to-sora -- \
  --jetton-master <ton_addr> \
  --sora-asset-id 0x<32-byte> \
  --recipient 0x<32-byte> \
  --amount <u128> \
  --nonce <u64> \
  --checkpoint-seqno <u32> --checkpoint-hash 0x<32-byte> \
  --target-seqno <u32> --target-hash 0x<32-byte> \
  --checkpoint-block-boc <0xhex|base64|@file> \
  --checkpoint-state-extra-proof <0xhex|base64|@file> \
  --target-block-proof-boc <0xhex|base64|@file> \
  --target-state-extra-proof <0xhex|base64|@file> \
  --shard-block-boc <0xhex|base64|@file> \
  --shard-state-accounts-proof <0xhex|base64|@file> \
  --account-proof <0xhex|base64|@file> \
  --burns-dict-proof <0xhex|base64|@file>
```

`--masterchain-proof` and `--shard-proof` remain available when you already have pre-encoded SCALE section bytes.

The encoder prints:

- `message_id`
- `proof_scale_hex`
- `proof_scale_base64`

`--ton-api <json-rpc-url>` can fill missing checkpoint/target block fields from `getMasterchainInfo`.

## Proof Inputs

Use the in-repo SCCP proof tooling described in [docs/sccp/PROOF_TOOLING.md](/Users/mtakemiya/dev/sora2-network/docs/sccp/PROOF_TOOLING.md) to generate:

- verifier bootstrap inputs for `SccpVerifierInitialize`
- finalized-root import inputs for `SccpVerifierSubmitSignatureCommitment`
- mint-proof JSON containing `proof`, `leaf`, and `digest_scale`

This command already outputs the verifier-ready proof cell BOC:
- `proof_cell_boc_hex`
- `proof_cell_boc_base64`

The helper script is still available for re-encoding historical or externally generated proof JSON:
- `node scripts/encode_sora_proof_cell.mjs --input ./mint-proof.json --format both`
- `npm run encode-proof-cell -- --input ./mint-proof.json --format both`

`scripts/deploy_mainnet.mjs` now emits the exact `SccpSetVerifier` body BOC in dry-run mode and, when you also provide the SORA-derived verifier bootstrap inputs, the exact `SccpVerifierInitialize` body BOC as well. In execute mode it can auto-send both governor-only bootstrap messages after deployment when the governor wallet mnemonic is available via `--governor-mnemonic-file`.
The deployed master remains `Paused` until SORA emits and TON verifies the canonical `addTokenFromProof` governance message for that asset.

## Build

```bash
npm install
npm run build
```

## Test

```bash
npm test
npm run test:formal-assisted
npm run test:formal-assisted:ci
npm run test:fuzz
npm run test:fuzz:nightly
npm run test:ci-fuzz
npm run test:ci-all
npm run test:deploy-scripts
npm run test:ci-formal
npm run check:repo-hygiene
./scripts/check_repo_hygiene.sh
./scripts/test_formal_assisted.sh
./scripts/test_fuzz_nightly.sh
./scripts/test_ci_formal.sh
./scripts/test_ci_fuzz.sh
./scripts/test_ci_all.sh
```

Artifacts are written to `artifacts/` as JSON (`*.compiled.json`) and include `codeBoc64`.

## Derive IDs For SORA Config

For SORA SCCP configuration for TON:
- `remote_token_id` (32 bytes) is the Jetton master **account-id** (`address.hash`) hex.
- `domain_endpoint` (32 bytes) can be set to the SCCP Jetton master **code hash** (`codeHashHex`) as a stable identifier.

Helper:

```bash
node scripts/derive_master_address.mjs --governor <legacy_seed_addr> --sora-asset-id <64hex>
```

`--governor` is still part of the address derivation because the one-time verifier bootstrap authority remains in init data, but it does not provide steady-state lifecycle control after deployment.
