# SORA Runtime Upgrade 4.8.8 Governance Parameters

Generated/verified: 2026-06-03T15:39:11Z
Checked against: `wss://ws.mof.sora.org` at block `26413341`

## Artifact

- Runtime package version: 4.8.8
- Runtime spec: sora-substrate
- Spec version: 130
- Transaction version: 130
- WASM file: `runtime-upgrade-4.8.8/framenode-runtime-4.8.8.compact.compressed.wasm`
- WASM bytes: 3037102
- WASM Blake2-256: `0x39f8609a748b13a0186e90023f709adf9d639bfa0777f6aaf3484a254e6b17b4`
- WASM SHA-256: `0xf1ea5ced3a614003a7a4209b77a00e5f8bcd66a6ba312375d2d17e2154b5dfb5`
- `system.set_code` encoded call file: `runtime-upgrade-4.8.8/set-code-call.scale`
- `system.set_code` encoded call bytes: 3037108
- `system.set_code` proposal hash: `0x6972400d4e5dc1c43566e5c9ca364207cabdf4125ff6fabce07d9f1417f2e55b`

## Preimage

Submit:

- Pallet/call: `Preimage.note_preimage`
- Argument: bytes from `runtime-upgrade-4.8.8/set-code-call.scale`
- Full encoded call hex file: `runtime-upgrade-4.8.8/preimage-note-call.hex`
- Full encoded call len: 3037114

After inclusion, verify the preimage exists for:

- Hash: `0x6972400d4e5dc1c43566e5c9ca364207cabdf4125ff6fabce07d9f1417f2e55b`
- Len: `3037108`

## Council Motion

Current live Council member count: 8.

Runtime origin required by `Democracy.external_propose_majority`: `AtLeastHalfCouncil`.

Submit:

- Pallet/call: `Council.propose`
- Threshold: `4` minimum for current 8-member council
- Conservative threshold: `5`
- Proposal: `Democracy.external_propose_majority`
- Proposal argument:
  - `Lookup.hash`: `0x6972400d4e5dc1c43566e5c9ca364207cabdf4125ff6fabce07d9f1417f2e55b`
  - `Lookup.len`: `3037108`
- Full `Council.propose` call hex, threshold 4: `runtime-upgrade-4.8.8/council-propose-external-majority-threshold-4-call.hex`
- Full `Council.propose` call hex, threshold 5: `runtime-upgrade-4.8.8/council-propose-external-majority-threshold-5-call.hex`
- Full `Council.propose` call len: 43
- Inner proposal hex file: `runtime-upgrade-4.8.8/democracy-external-propose-majority-call.hex`
- Inner proposal hex: `0x1d05026972400d4e5dc1c43566e5c9ca364207cabdf4125ff6fabce07d9f1417f2e55bb4572e00`
- Inner proposal hash: `0x06998b7a2fe740c9070c68d82c8dfffc6a5633fa2c39f6ba71fd3707d5c6fab1`
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
  - `proposal_hash`: `0x6972400d4e5dc1c43566e5c9ca364207cabdf4125ff6fabce07d9f1417f2e55b`
  - `voting_period`: `1800`
  - `delay`: `0`
- Voting period note: 1800 blocks, about 3 hours at 6 second blocks
- Full `TechnicalCommittee.propose` call hex, threshold 3: `runtime-upgrade-4.8.8/technical-committee-fast-track-threshold-3-call.hex`
- Full `TechnicalCommittee.propose` call len: 46
- Inner proposal hex file: `runtime-upgrade-4.8.8/democracy-fast-track-call.hex`
- Inner proposal hex: `0x1d076972400d4e5dc1c43566e5c9ca364207cabdf4125ff6fabce07d9f1417f2e55b0807000000000000`
- Inner proposal hash: `0x2cbcf6daf34d9366a5ef557c2a255e10c027b195d60204d7fff597e67fb78b0b`
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

The rehearsal scraped live state from `wss://ws.mof.sora.org`, found 1,182,730 keys,
executed the runtime upgrade replay, and finished with `1 passed; 0 failed` in 1838.69s.

## Helper Commands

Use the signing account for each body.

Note preimage:

```bash
misc/runtime_upgrade/.venv/bin/python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --mnemonic "$MNEMONIC" \
  note-preimage \
  --call-file-path runtime-upgrade-4.8.8/set-code-call.scale
```

Council external proposal, minimum valid threshold for current membership:

```bash
misc/runtime_upgrade/.venv/bin/python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --mnemonic "$MNEMONIC" \
  council-propose-majority \
  --preimage-json runtime-upgrade-4.8.8/preimage.json \
  --threshold 4
```

Technical committee fast-track:

```bash
misc/runtime_upgrade/.venv/bin/python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --mnemonic "$MNEMONIC" \
  tech-fast-track \
  --preimage-json runtime-upgrade-4.8.8/preimage.json \
  --threshold 3 \
  --voting-period 1800 \
  --delay 0
```
