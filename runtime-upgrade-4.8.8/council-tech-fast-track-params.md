# SORA Runtime Upgrade 4.8.8 Governance Parameters

Generated/verified: 2026-06-04T11:21:10Z
Checked against: `wss://ws.mof.sora.org` at block `26423770`

## Artifact

- Runtime package version: 4.8.8
- Runtime spec: sora-substrate
- Spec version: 130
- Transaction version: 130
- WASM file: `runtime-upgrade-4.8.8/framenode-runtime-4.8.8.compact.compressed.wasm`
- WASM bytes: 3037507
- WASM Blake2-256: `0x2b33b01ba3f9e58e269b0e9619d25bedf3f60cdd6ed0ec519dacc75259c85d1e`
- WASM SHA-256: `0x0ac7b85d845d56d450df99ab7166014897be57a7a1f66437d92e6949b56f49c0`
- `system.set_code` encoded call file: `runtime-upgrade-4.8.8/set-code-call.scale`
- `system.set_code` encoded call bytes: 3037513
- `system.set_code` proposal hash: `0xd42473f3cbb896dc0b0bcc1c7d0929bdfe04303c53387b92b56937137dbb2c50`

## Preimage

Submit:

- Pallet/call: `Preimage.note_preimage`
- Argument: bytes from `runtime-upgrade-4.8.8/set-code-call.scale`
- Full encoded call hex file: `runtime-upgrade-4.8.8/preimage-note-call.hex`
- Full encoded call len: 3037519

After inclusion, verify the preimage exists for:

- Hash: `0xd42473f3cbb896dc0b0bcc1c7d0929bdfe04303c53387b92b56937137dbb2c50`
- Len: `3037513`

## Council Motion

Current live Council member count: 8.

Runtime origin required by `Democracy.external_propose_majority`: `AtLeastHalfCouncil`.

Submit:

- Pallet/call: `Council.propose`
- Threshold: `4` minimum for current 8-member council
- Conservative threshold: `5`
- Proposal: `Democracy.external_propose_majority`
- Proposal argument:
  - `Lookup.hash`: `0xd42473f3cbb896dc0b0bcc1c7d0929bdfe04303c53387b92b56937137dbb2c50`
  - `Lookup.len`: `3037513`
- Full `Council.propose` call hex, threshold 4: `runtime-upgrade-4.8.8/council-propose-external-majority-threshold-4-call.hex`
- Full `Council.propose` call hex, threshold 5: `runtime-upgrade-4.8.8/council-propose-external-majority-threshold-5-call.hex`
- Full `Council.propose` call len: 43
- Inner proposal hex file: `runtime-upgrade-4.8.8/democracy-external-propose-majority-call.hex`
- Inner proposal hex: `0x1d0502d42473f3cbb896dc0b0bcc1c7d0929bdfe04303c53387b92b56937137dbb2c5049592e00`
- Inner proposal hash: `0xbad482f684c594f8fd7633736013240cb08ca8ea61826a87fcaacb0b4514799c`
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
  - `proposal_hash`: `0xd42473f3cbb896dc0b0bcc1c7d0929bdfe04303c53387b92b56937137dbb2c50`
  - `voting_period`: `1800`
  - `delay`: `0`
- Voting period note: 1800 blocks, about 3 hours at 6 second blocks
- Full `TechnicalCommittee.propose` call hex, threshold 3: `runtime-upgrade-4.8.8/technical-committee-fast-track-threshold-3-call.hex`
- Full `TechnicalCommittee.propose` call len: 46
- Inner proposal hex file: `runtime-upgrade-4.8.8/democracy-fast-track-call.hex`
- Inner proposal hex: `0x1d07d42473f3cbb896dc0b0bcc1c7d0929bdfe04303c53387b92b56937137dbb2c500807000000000000`
- Inner proposal hash: `0x9402be246c3a4cf50bab77c6e3a135a4483cd6ad73127c7b49e09c65a11bbe7c`
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

Attempted during 4.8.8 validation.

- Full all-pallet `./misc/runtime_upgrade/run_remote_try_runtime.sh` stalled during remote state scraping.
- Polkamarkt live-state rehearsal passed with:
  `SNAP=/tmp/sora-polkamarkt-remote.snap REMOTE_PALLETS=System,Polkamarkt REMOTE_CHILD_TRIE=0 ./misc/runtime_upgrade/run_remote_try_runtime.sh`
  This downloaded 33,594 `System` keys and 29 `Polkamarkt` keys from `wss://ws.mof.sora.org`, replayed the snapshot, and completed with `1 passed; 0 failed`.
- Broader selected-pallet run with `REMOTE_CHILD_TRIE=0` progressed through selected prefixes up to `Polkamarkt`, including 22,616-key, 17,313-key, 15,959-key, 8,351-key, 9,207-key, and 29-key `Polkamarkt` prefixes, then stalled on the following large prefix, likely `EthBridge`, before migration assertions completed.
- Focused `EthBridge` live-state rehearsal passed with:
  `REQUIRE_REMOTE=1 REMOTE_RPC_URL=ws://127.0.0.1:9946 SNAP=/tmp/sora-ethbridge-focused.snap cargo test -p framenode-runtime --features try-runtime remote_eth_bridge_migration_rehearsal -- --exact --nocapture`
  The local WS endpoint proxied JSON-RPC to `https://ws.mof.sora.org`. This downloaded 228,562 `EthBridge::RequestStatuses` keys, 228,562 `EthBridge::Requests` keys, and `EthBridge::StorageVersion`; it completed with `1 passed; 0 failed` in 562.43s. Offline snapshot replay from `/tmp/sora-ethbridge-focused.snap` completed with `1 passed; 0 failed` in 29.97s.
- Filtered release-scope Executive rehearsal passed with:
  `SNAP=/tmp/sora-selected-plus-ethbridge-focused.snap REQUIRE_REMOTE=1 REMOTE_RPC_URL=ws://127.0.0.1:9946 REMOTE_CHILD_TRIE=0 REMOTE_PALLETS=XorFee,Staking,Offences,Session,Grandpa,ImOnline,PoolXYK,PswapDistribution,VestedRewards,Identity,Farming,Kensetsu,Band,Polkamarkt,OracleProxy,BridgeInboundChannel,SubstrateBridgeInboundChannel,SubstrateBridgeOutboundChannel REMOTE_HASHED_PREFIXES=0x8e8f4d4777099074419cbb4597dac1d35c63ad4ba9b87d8a9938e57f2391a87b,0x8e8f4d4777099074419cbb4597dac1d3e973821932a99de497c2362ed9f9be34 REMOTE_HASHED_KEYS=0x8e8f4d4777099074419cbb4597dac1d3308ce9615de0775a82f8a94dc3d285a1 ./misc/runtime_upgrade/run_remote_try_runtime.sh`
  This ran the normal `Executive::execute_on_runtime_upgrade()` path against selected live state for the 4.8.8 migration tuple, replacing the full `EthBridge` pallet prefix with the two maps touched by the v0-to-v3 migration plus the storage-version key. It completed with `1 passed; 0 failed` in 675.82s, and offline snapshot replay from `/tmp/sora-selected-plus-ethbridge-focused.snap` completed with `1 passed; 0 failed` in 14.78s.
- All-80 runtime pallet prefix attempt:
  `SNAP=/tmp/sora-all-pallet-prefixes.snap REQUIRE_REMOTE=1 REMOTE_RPC_URL=ws://127.0.0.1:9946 REMOTE_CHILD_TRIE=0 REMOTE_PALLETS=<all pallet names from runtime-upgrade-4.8.8/framenode-runtime-4.8.8-metadata.json> ./misc/runtime_upgrade/run_remote_try_runtime.sh`
  The HTTP-proxied run loaded the full `EthBridge` pallet prefix successfully, including 744,813 live keys, and progressed through many smaller later prefixes. It was stopped after roughly 48 minutes while still scraping a later pallet prefix; no `/tmp/sora-all-pallet-prefixes.snap` snapshot was produced, so migration assertions did not run.
- Remaining caveat: an unrestricted all-pallet scrape still has not completed. Direct RPC counting found 744,813 live keys under the full `EthBridge` pallet prefix and 116,912 live keys under `BridgeProxy`, which makes the unfiltered all-pallet remote-externalities scrape impractical for this release gate.

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
