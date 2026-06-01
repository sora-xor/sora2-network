# SORA Runtime Upgrade 4.8.7 Governance Parameters

Generated/verified: 2026-06-01T18:08:20Z
Checked against: `wss://ws.mof.sora.org` at block `26388052`

## Artifact

- Runtime package version: 4.8.7
- Runtime spec: sora-substrate
- Spec version: 129
- Transaction version: 129
- WASM file: `runtime-upgrade-4.8.7/framenode-runtime-4.8.7.compact.compressed.wasm`
- WASM bytes: 3054436
- WASM Blake2-256: `0x3afe694c60d21952900c4a823f85198140e2975a5657f33ed622d37e5c6cb3e6`
- WASM SHA-256: `0x1a3ea901ae435f3c33c842f520828aff1909c0bfc2539077bf790368be8b433e`
- `system.set_code` encoded call file: `runtime-upgrade-4.8.7/set-code-call.scale`
- `system.set_code` encoded call bytes: 3054442
- `system.set_code` proposal hash: `0x6777170da23f4c55e0ac091542ff8f80e7d5febb0113ed36d395f2c64887bec1`

## Preimage

Submit:

- Pallet/call: `Preimage.note_preimage`
- Argument: bytes from `runtime-upgrade-4.8.7/set-code-call.scale`
- Full encoded call hex file: `runtime-upgrade-4.8.7/preimage-note-call.hex`
- Full encoded call len: 3054448

After inclusion, verify the preimage exists for:

- Hash: `0x6777170da23f4c55e0ac091542ff8f80e7d5febb0113ed36d395f2c64887bec1`
- Len: `3054442`

## Council Motion

Current live Council member count: 8.

Runtime origin required by `Democracy.external_propose_majority`: `AtLeastHalfCouncil`.

Submit:

- Pallet/call: `Council.propose`
- Threshold: `4` minimum for current 8-member council
- Conservative threshold: `5`
- Proposal: `Democracy.external_propose_majority`
- Proposal argument:
  - `Lookup.hash`: `0x6777170da23f4c55e0ac091542ff8f80e7d5febb0113ed36d395f2c64887bec1`
  - `Lookup.len`: `3054442`
- Full `Council.propose` call hex, threshold 4: `runtime-upgrade-4.8.7/council-propose-external-majority-threshold-4-call.hex`
- Full `Council.propose` call hex, threshold 5: `runtime-upgrade-4.8.7/council-propose-external-majority-threshold-5-call.hex`
- Full `Council.propose` call len: 43
- Inner proposal hex file: `runtime-upgrade-4.8.7/democracy-external-propose-majority-call.hex`
- Inner proposal hex: `0x1d05026777170da23f4c55e0ac091542ff8f80e7d5febb0113ed36d395f2c64887bec16a9b2e00`
- Inner proposal hash: `0x52a0b7851c0857cd97ca0640cdf21ac5b1acbb37a68e979986cb3c0583cfed3c`
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
  - `proposal_hash`: `0x6777170da23f4c55e0ac091542ff8f80e7d5febb0113ed36d395f2c64887bec1`
  - `voting_period`: `1800`
  - `delay`: `0`
- Voting period note: 1800 blocks, about 3 hours at 6 second blocks
- Full `TechnicalCommittee.propose` call hex, threshold 3: `runtime-upgrade-4.8.7/technical-committee-fast-track-threshold-3-call.hex`
- Full `TechnicalCommittee.propose` call len: 46
- Inner proposal hex file: `runtime-upgrade-4.8.7/democracy-fast-track-call.hex`
- Inner proposal hex: `0x1d076777170da23f4c55e0ac091542ff8f80e7d5febb0113ed36d395f2c64887bec10807000000000000`
- Inner proposal hash: `0x14721b4cf07bca6b1b34b0a004f3c5fe2c4e8c9e8ad5c1aa1917088430e091d9`
- `length_bound`: `42`

## Current Chain State Check

Checked against `wss://ws.mof.sora.org`:

- `Democracy.FastTrackVotingPeriod`: 1800
- `Democracy.LaunchPeriod`: 403200
- `Democracy.VotingPeriod`: 201600
- `Democracy.EnactmentPeriod`: 432000
- `Democracy.NextExternal`: `None`

If council or technical committee membership changes before submission, recompute the thresholds.

## Remote Rehearsal

Passed:

```bash
./misc/runtime_upgrade/run_remote_try_runtime.sh
```

The rehearsal scraped live state from `https://ws.mof.sora.org`, executed the runtime upgrade, and verified final storage versions.

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
