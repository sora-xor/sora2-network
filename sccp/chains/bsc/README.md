# sccp-bsc

Migration note: this package still describes and implements the pre-Nexus SORA BEEFY/MMR verifier
model. The active SCCP hub is now Nexus in `../iroha`; treat this directory as pending downstream
verifier migration.

Current Nexus-native surfaces in this repo:
- `npm run build-router-call-from-nexus-bundle`
- `npm run test:nexus-bundle-router-call`
- `scripts/sccp_e2e_adapter.sh mint_verify` consumes `SCCP_HUB_BUNDLE_*` artifacts when present

The legacy `SoraBeefyLightClientVerifier` contract remains in-tree for historical/reference work,
but it is not the active E2E or operator-tooling path anymore.

SORA Cross-Chain Protocol (SCCP) contracts for EVM chains (BSC).

See `contracts/` for:
- `SccpRouter`: burns wrapped tokens, mints from SORA-finalized burn proofs, and applies token lifecycle updates (`add`, `pause`, `resume`) only from SORA-finalized governance proofs.
- `SccpToken`: minimal ERC-20 wrapper token for a SORA asset.
- `ISccpVerifier`: pluggable verifier interface for finalized burn/governance messages.
- `SoraBeefyLightClientVerifier`: BEEFY+MMR light client used to verify SORA commitments.

The design is roleless on this chain:
- no local governor/operator controls,
- immutable verifier on router construction,
- token add/pause/resume only via proven SORA governance messages.
- outbound BSC burns are the canonical proof target for trustless BSC -> SORA proof generation.

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
npm run test:cli-helpers
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

## Deploy (high level)

1. Deploy `SoraBeefyLightClientVerifier` with constructor bootstrap data from SORA:
   - `latestBeefyBlock`
   - `currentValidatorSet = { id, len, root }`
   - `nextValidatorSet = { id, len, root }`
2. Deploy `SccpRouter(localDomain, verifier)` with:
   - `localDomain = 2` (BSC)
   - `verifier = <SoraBeefyLightClientVerifier>`
3. Point SORA SCCP domain endpoint to the new router.
4. Add bridgeable assets by submitting SORA-finalized `TokenAddPayloadV1` proofs to `addTokenFromProof`.

## Mint (Any -> BSC, Via SORA Finality)

Minting on this chain is driven by finalized SORA commitments:
- `SccpRouter.mintFromProof(sourceDomain, payload, soraBeefyMmrProof)`
- router verifies payload/message binding and proof finality via `ISccpVerifier.verifyBurnProof`.

## Burn (BSC -> SORA, Proof Target)

Canonical outbound `BSC -> SORA` burn proof target:

- `SccpRouter.SccpBurned(messageId, soraAssetId, sender, amount, destDomain, recipient, nonce, payload)`
- `SccpRouter.BURN_EVENT_TOPIC0()` exposes the canonical event topic hash
- `SccpRouter.burnPayload(messageId)` reconstructs the canonical payload bytes from storage

Reference tooling for UI-owned proof generation:

```bash
npm run extract-burn-proof-inputs -- \
  --receipt-file ./burn-receipt.json \
  --router 0x<router_address>

npm run build-burn-proof-to-sora -- \
  --rpc-url https://<BSC_RPC_URL> \
  --router 0x<router_address> \
  --payload 0x<burn_payload_hex> \
  --block <block_number>

npm run build-bsc-header-rlp -- \
  --rpc-url https://<BSC_RPC_URL> \
  --block-number <checkpoint_block_number> \
  --bsc-epoch-length <epoch_length>
```

The trustless BSC -> SORA flow is:

1. user burns on `SccpRouter.burnToDomain(...)`
2. UI extracts the canonical `payload` / `messageId` from the `SccpBurned` receipt
3. UI requests `eth_getProof` for `burns[messageId].sender` and builds SCALE `EvmBurnProofV1`
4. UI submits `sccp.mint_from_proof(...)` or `sccp.attest_burn(...)` on SORA with those bytes

There is no supported SCCP attester fallback for `BSC -> SORA`; unsupported fallback modes fail closed.

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
  --target-domain 2 \
  --nonce 1 \
  --sora-asset-id 0x<assetId32> \
  --decimals 18 \
  --name "SCCP Wrapped" \
  --symbol "wSORA"
```

## Verifier Security Properties (SORA -> BSC)

`SoraBeefyLightClientVerifier` enforces:
- `>= 2/3` validator signatures per imported BEEFY commitment,
- validator merkle-membership proofs against set root,
- duplicate signer-key rejection,
- ECDSA validity checks (`r != 0`, `s != 0`, low-`s`),
- proof fail-closed behavior for malformed payload/proof bytes.

## Proof Generation (SORA -> BSC)

Use the in-repo SCCP proof tooling to build verifier inputs for the opposite direction, `SORA -> BSC`:

1. Export verifier init sets.
2. Import finalized SORA MMR roots.
3. Build ABI payload/proof bytes for burn or governance message types.

The on-chain verifier consumes proof bytes shaped as:
- `abi.encode(uint64 leafIndex, uint64 leafCount, bytes32[] items, MmrLeaf leaf, bytes digestScale)`
