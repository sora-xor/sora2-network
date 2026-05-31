# SORA Runtime Upgrade 4.8.7 Governance Parameters

Generated/verified: 2026-05-31T15:37:20Z

## Artifact

- Runtime package version: 4.8.7
- Runtime spec: sora-substrate
- Spec version: 129
- Transaction version: 129
- WASM file: `runtime-upgrade-4.8.7/framenode-runtime-4.8.7.compact.compressed.wasm`
- WASM bytes: 3002787
- WASM Blake2-256: `0x8287dfb031892936c5f2e5fbad37a173c7919851d1a278bdfa2580cfd2895419`
- WASM SHA-256: `0x7cc28bad4bf772827fea28392e9f884626858c391af039d53358e6200f8952cf`
- `system.set_code` encoded call file: `runtime-upgrade-4.8.7/set-code-call.scale`
- `system.set_code` encoded call bytes: 3002793
- `system.set_code` proposal hash: `0x13409c3bbeb440cf02088f9b458f728e8a9ca744585f88eff77cb5c38fd348c6`

## Preimage

Submit:

- Pallet/call: `Preimage.note_preimage`
- Argument: bytes from `runtime-upgrade-4.8.7/set-code-call.scale`
- Full encoded call hex file: `runtime-upgrade-4.8.7/preimage-note-call.hex`
- Full encoded call len: 3002799

After inclusion, verify the preimage exists for:

- Hash: `0x13409c3bbeb440cf02088f9b458f728e8a9ca744585f88eff77cb5c38fd348c6`
- Len: 3002793

## Council Motion

Current live Council member count: 8.

Runtime origin required by `Democracy.external_propose_majority`: `AtLeastHalfCouncil`.

Submit:

- Pallet/call: `Council.propose`
- Threshold: `4` minimum for current 8-member council
- Conservative threshold: `5`
- Proposal: `Democracy.external_propose_majority`
- Proposal argument:
  - `Lookup.hash`: `0x13409c3bbeb440cf02088f9b458f728e8a9ca744585f88eff77cb5c38fd348c6`
  - `Lookup.len`: `3002793`
- Full `Council.propose` call hex, threshold 4: `runtime-upgrade-4.8.7/council-propose-external-majority-threshold-4-call.hex`
- Full `Council.propose` call hex, threshold 5: `runtime-upgrade-4.8.7/council-propose-external-majority-threshold-5-call.hex`
- Full `Council.propose` call len: 43
- Inner proposal hex file: `runtime-upgrade-4.8.7/democracy-external-propose-majority-call.hex`
- Inner proposal hex: `0x1d050213409c3bbeb440cf02088f9b458f728e8a9ca744585f88eff77cb5c38fd348c6a646b700`
- Inner proposal hash: `0x9cbcb72beeb5ab51ca1623cfe91b1505671bd2642df66259a9028de2bc5032cd`
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
  - `proposal_hash`: `0x13409c3bbeb440cf02088f9b458f728e8a9ca744585f88eff77cb5c38fd348c6`
  - `voting_period`: `1800`
  - `delay`: `0`
- Voting period note: 1800 blocks, about 3 hours at 6 second blocks
- Full `TechnicalCommittee.propose` call hex, threshold 3: `runtime-upgrade-4.8.7/technical-committee-fast-track-threshold-3-call.hex`
- Full `TechnicalCommittee.propose` call len: 46
- Inner proposal hex file: `runtime-upgrade-4.8.7/democracy-fast-track-call.hex`
- Inner proposal hex: `0x1d0713409c3bbeb440cf02088f9b458f728e8a9ca744585f88eff77cb5c38fd348c60807000000000000`
- Inner proposal hash: `0xfd09c0bf9c8c107b5ca01cbad2d4054f04e04193018728c57169dacf31904585`
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
