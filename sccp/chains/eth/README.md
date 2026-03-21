# sccp-eth

SORA Cross-Chain Protocol (SCCP) contracts for EVM chains (ETH).

See `contracts/` for:
- `SccpRouter`: burns wrapped tokens, exposes the canonical outbound burn proof target log, mints from SORA-finalized burn proofs, and applies token lifecycle updates (`add`, `pause`, `resume`) only from SORA-finalized governance proofs.
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
```

## Deploy (high level)

1. Deploy `SoraBeefyLightClientVerifier` with constructor bootstrap data from SORA:
   - `latestBeefyBlock`
   - `currentValidatorSet = { id, len, root }`
   - `nextValidatorSet = { id, len, root }`
2. Deploy `SccpRouter(localDomain, verifier)` with:
   - `localDomain = 1` (ETH)
   - `verifier = <SoraBeefyLightClientVerifier>`
3. Point SORA SCCP domain endpoint to the new router.
4. Add bridgeable assets by submitting SORA-finalized `TokenAddPayloadV1` proofs to `addTokenFromProof`.

## Mint (Any -> ETH, Via SORA Finality)

Minting on this chain is driven by finalized SORA commitments:
- `SccpRouter.mintFromProof(sourceDomain, payload, soraBeefyMmrProof)`
- router verifies payload/message binding and proof finality via `ISccpVerifier.verifyBurnProof`.

## Burn (ETH -> SORA, Proof Target)

Outbound burns use the router log as the canonical proof target:
- `SccpRouter.SccpBurned(messageId, soraAssetId, sender, amount, destDomain, recipient, nonce, payload)`
- `SccpRouter.BURN_EVENT_TOPIC0()` exposes the canonical event topic hash
- `SccpRouter.burnPayload(messageId)` reconstructs the canonical payload bytes from storage

Trustless proof generation is expected to happen off-chain in the user UI or prover.
This repo now provides a canonical receipt-to-public-input extractor for that flow:

```bash
npm run extract-burn-proof-inputs -- \
  --receipt-file /path/to/eth-burn-receipt.json \
  --router 0x<routerAddress>
```

The extractor validates that:
- the selected receipt log is `SccpBurned`,
- `messageId == keccak256("sccp:burn:v1" || payload)`,
- the event fields match the encoded burn payload bytes.

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
  --target-domain 1 \
  --nonce 1 \
  --sora-asset-id 0x<assetId32> \
  --decimals 18 \
  --name "SCCP Wrapped" \
  --symbol "wSORA"
```

## Verifier Security Properties (SORA -> ETH)

`SoraBeefyLightClientVerifier` enforces:
- `>= 2/3` validator signatures per imported BEEFY commitment,
- validator merkle-membership proofs against set root,
- duplicate signer-key rejection,
- ECDSA validity checks (`r != 0`, `s != 0`, low-`s`),
- proof fail-closed behavior for malformed payload/proof bytes.

## Proof Generation

Use the in-repo SCCP proof tooling to build verifier inputs:

1. Export verifier init sets.
2. Import finalized SORA MMR roots.
3. Build ABI payload/proof bytes for burn or governance message types.

The on-chain verifier consumes proof bytes shaped as:
- `abi.encode(uint64 leafIndex, uint64 leafCount, bytes32[] items, MmrLeaf leaf, bytes digestScale)`

There is no supported SCCP attester fallback for `ETH -> SORA`; unsupported fallback modes fail closed.
