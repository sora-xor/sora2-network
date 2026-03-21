# sccp-tron

SORA Cross-Chain Protocol (SCCP) contracts for TRON.

See `contracts/` for:
- `SccpRouter`: burns wrapped tokens, exposes the canonical outbound burn export surface for `TRON -> SORA`, mints from SORA-finalized burn proofs, and applies token lifecycle updates (`add`, `pause`, `resume`) only from SORA-finalized governance proofs.
- `SccpToken`: minimal ERC-20 wrapper token for a SORA asset.
- `ISccpVerifier`: pluggable verifier interface for finalized burn/governance messages.
- `SoraBeefyLightClientVerifier`: BEEFY+MMR light client used to verify SORA commitments.

The design is roleless on this chain:
- no local governor/operator controls,
- immutable verifier on router construction,
- token add/pause/resume only via proven SORA governance messages.

Runtime/tooling:
- Node.js 22 (`.nvmrc`, `package.json#engines`)

## Build

```bash
./scripts/compile.sh
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
./scripts/sync_ci_assets.sh
```

## Deploy (high level)

1. Deploy `SoraBeefyLightClientVerifier` with constructor bootstrap data from SORA:
   - `latestBeefyBlock`
   - `currentValidatorSet = { id, len, root }`
   - `nextValidatorSet = { id, len, root }`
2. Deploy `SccpRouter(localDomain, verifier)` with:
   - `localDomain = 5` (TRON)
   - `verifier = <SoraBeefyLightClientVerifier>`
3. Point SORA SCCP domain endpoint to the new router.
4. Add bridgeable assets by submitting SORA-finalized `TokenAddPayloadV1` proofs to `addTokenFromProof`.

## Mint (Any -> TRON, Via SORA Finality)

Minting on this chain is driven by finalized SORA commitments:
- `SccpRouter.mintFromProof(sourceDomain, payload, soraBeefyMmrProof)`
- router verifies payload/message binding and proof finality via `ISccpVerifier.verifyBurnProof`.

## Burn (TRON -> SORA, Canonical Export Surface)

Outbound burns use the router burn record as the canonical export target:
- `SccpRouter.SccpBurned(messageId, soraAssetId, sender, amount, destDomain, recipient, nonce, payload)`
- `SccpRouter.BURN_EVENT_TOPIC0()` exposes the canonical event topic hash
- `SccpRouter.burnPayload(messageId)` reconstructs the canonical payload bytes from storage

This repo provides a receipt extractor for that burn surface:

```bash
npm run extract-burn-export -- \
  --receipt-file /path/to/tron-burn-receipt.json \
  --router 0x<routerAddress>
```

The first-class entrypoint is `npm run extract-burn-export`.
`npm run extract-burn-proof-inputs` remains as a compatibility alias for the same extractor.

The extractor emits:
- `schema = "sccp-tron-burn-export/v1"`
- `export_surface = { router, event_topic0, message_id, payload_hex, source_domain, dest_domain }`

Compatibility metadata remains for older consumers:
- `schema_aliases = ["sccp-tron-burn-proof-inputs/v1"]`
- `proof_public_inputs` is preserved as a deprecated alias of `export_surface`

The extractor validates that:
- the selected receipt log is `SccpBurned`,
- `messageId == keccak256("sccp:burn:v1" || payload)`,
- the event fields match the encoded payload bytes,
- `payload.source_domain == 5` and `payload.dest_domain == 0`.

This repo does not implement a trustless SORA-side verifier path for `TRON -> SORA`.
A trustless flow would require verifier support in `sora2-network` such as a TRON light client,
an anchored TRON-state verifier, or a zk verifier that SORA can check on-chain.

## Token Lifecycle (Proof-Driven)

All lifecycle controls are proof-driven and replay-protected:
- `addTokenFromProof(payload, proof)`
- `pauseTokenFromProof(payload, proof)`
- `resumeTokenFromProof(payload, proof)`

Each action uses a distinct message type/prefix and verifier method.

Encode governance payload bytes and message id off-chain:

```bash
npm run encode-governance-payload -- \
  --action add \
  --target-domain 5 \
  --nonce 1 \
  --sora-asset-id 0x<assetId32> \
  --decimals 18 \
  --name "SCCP Wrapped" \
  --symbol "wSORA"
```

## Verifier Security Properties (SORA -> TRON)

`SoraBeefyLightClientVerifier` enforces:
- `>= 2/3` validator signatures per imported BEEFY commitment,
- validator merkle-membership proofs against set root,
- duplicate signer-key rejection,
- ECDSA validity checks (`r != 0`, `s != 0`, low-`s`),
- proof fail-closed behavior for malformed payload/proof bytes.

## SORA -> TRON Proof Generation

Use the in-repo SCCP proof tooling to build verifier inputs:

1. Export verifier init sets.
2. Import finalized SORA MMR roots.
3. Build ABI payload/proof bytes for burn or governance message types.

The on-chain verifier consumes proof bytes shaped as:
- `abi.encode(uint64 leafIndex, uint64 leafCount, bytes32[] items, MmrLeaf leaf, bytes digestScale)`

There is no supported SCCP attester fallback for `TRON -> SORA`; unsupported fallback modes fail closed.
