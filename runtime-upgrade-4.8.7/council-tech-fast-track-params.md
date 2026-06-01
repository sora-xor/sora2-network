# SORA Runtime Upgrade 4.8.7 Governance Parameters

Generated/verified: 2026-06-01T11:12:39Z

## Artifact

- Runtime package version: 4.8.7
- Runtime spec: sora-substrate
- Spec version: 129
- Transaction version: 129
- WASM file: `runtime-upgrade-4.8.7/framenode-runtime-4.8.7.compact.compressed.wasm`
- WASM bytes: 3018668
- WASM Blake2-256: `0xe606d842d0b6f4eba35617c26ea21aefabc249a823c3ba05ad9c9f46fc687e80`
- WASM SHA-256: `0x908f6681a14d6a044df15b46c8015d9763b5fb8608bf3fdfc6a69b62722a437a`
- `system.set_code` encoded call file: `runtime-upgrade-4.8.7/set-code-call.scale`
- `system.set_code` encoded call bytes: 3018674
- `system.set_code` proposal hash: `0xa9cc2ad7754274e18861caad35a8bdb980637843e6b90515a7e6f635db3b7cdb`

## Preimage

Submit:

- Pallet/call: `Preimage.note_preimage`
- Argument: bytes from `runtime-upgrade-4.8.7/set-code-call.scale`
- Full encoded call hex file: `runtime-upgrade-4.8.7/preimage-note-call.hex`
- Full encoded call len: 3018680

After inclusion, verify the preimage exists for:

- Hash: `0xa9cc2ad7754274e18861caad35a8bdb980637843e6b90515a7e6f635db3b7cdb`
- Len: 3018674

## Council Motion

Current live Council member count: 8.

Runtime origin required by `Democracy.external_propose_majority`: `AtLeastHalfCouncil`.

Submit:

- Pallet/call: `Council.propose`
- Threshold: `4` minimum for current 8-member council
- Conservative threshold: `5`
- Proposal: `Democracy.external_propose_majority`
- Proposal argument:
  - `Lookup.hash`: `0xa9cc2ad7754274e18861caad35a8bdb980637843e6b90515a7e6f635db3b7cdb`
  - `Lookup.len`: `3018674`
- Full `Council.propose` call hex, threshold 4: `runtime-upgrade-4.8.7/council-propose-external-majority-threshold-4-call.hex`
- Full `Council.propose` call hex, threshold 5: `runtime-upgrade-4.8.7/council-propose-external-majority-threshold-5-call.hex`
- Full `Council.propose` call len: 43
- Inner proposal hex file: `runtime-upgrade-4.8.7/democracy-external-propose-majority-call.hex`
- Inner proposal hex: `0x1d0502a9cc2ad7754274e18861caad35a8bdb980637843e6b90515a7e6f635db3b7cdbb20f2e00`
- Inner proposal hash: `0xe2d6f49dbdbfd116337a7a08e087825317ee9f7d557c261bc1df9ec88fb91cf0`
- `length_bound`: `39`

Wait for the council motion to execute before submitting technical committee fast-track.

## Technical Committee Motion

Current live TechnicalCommittee member count: 4.

Runtime origin required by `Democracy.fast_track`: more than half of TechnicalCommittee.

Submit:

- Pallet/call: `TechnicalCommittee.propose`
- Threshold: `3`
- Proposal: `Democracy.fast_track`
- Proposal arguments:
  - `proposal_hash`: `0xa9cc2ad7754274e18861caad35a8bdb980637843e6b90515a7e6f635db3b7cdb`
  - `voting_period`: `1800`
  - `delay`: `0`
- Voting period note: 1800 blocks, about 3 hours at 6 second blocks
- Full `TechnicalCommittee.propose` call hex, threshold 3: `runtime-upgrade-4.8.7/technical-committee-fast-track-threshold-3-call.hex`
- Full `TechnicalCommittee.propose` call len: 46
- Inner proposal hex file: `runtime-upgrade-4.8.7/democracy-fast-track-call.hex`
- Inner proposal hex: `0x1d07a9cc2ad7754274e18861caad35a8bdb980637843e6b90515a7e6f635db3b7cdb0807000000000000`
- Inner proposal hash: `0xbd07743e7b71e79b5fcc28b7a9077658378554736d65fbb68690704a0eb84289`
- `length_bound`: `42`

## Current Chain State Check

Checked against `wss://ws.mof.sora.org`:

- `Democracy.FastTrackVotingPeriod`: 1800
- `Democracy.LaunchPeriod`: 403200
- `Democracy.VotingPeriod`: 201600
- `Democracy.EnactmentPeriod`: 432000
- `Democracy.NextExternal`: `None`

If council or technical committee membership changes before submission, recompute the thresholds.

## Helper Commands

Use the signing account for each body.

Note preimage:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --mnemonic "$MNEMONIC" \
  note-preimage \
  --call-file-path runtime-upgrade-4.8.7/set-code-call.scale
```

Council external proposal, minimum valid threshold for current membership:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --mnemonic "$MNEMONIC" \
  council-propose-majority \
  --preimage-json runtime-upgrade-4.8.7/preimage.json \
  --threshold 4
```

Technical committee fast-track:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --mnemonic "$MNEMONIC" \
  tech-fast-track \
  --preimage-json runtime-upgrade-4.8.7/preimage.json \
  --threshold 3 \
  --voting-period 1800 \
  --delay 0
```
