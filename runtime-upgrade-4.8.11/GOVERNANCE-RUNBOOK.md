# Activate the SORA 4.8.11 staking reward fix

Candidate SHA-256:
`15310a2f7899d458c84e47278c78f075fec74e3d6ab3de1fdc20ef914b14960a`.

Runtime proposal hash:
`0xc1d1b2e68fae6166a911f595ca2abc157afc97a12b14221191f9f6bfba6a9348`.
This hashes the **3,055,449-byte** SCALE `system.setCode` call, including the
validated Wasm. All `.hex` files below contain unsigned calls.

## Check the package and current chain

From `runtime-upgrade-4.8.11/validation`, install locked tooling with `npm ci`.
Verify the unchanged package offline with:

```sh
python3 verify-package.py --check-workspace-source
node verify-governance-calls.cjs
```

Before signing, inspect finalized state again using `prepare-package.cjs` and
`prepare-governance-calls.cjs`. These helpers allow public read-only RPCs and
encode unsigned calls; they never sign or submit. They require the tested
mainnet genesis, exact deployed spec-132 baseline and absent publication marker.
The governance helper rejects an occupied external queue or candidate blacklist
entry. If a precondition changes, review and repeat validation against the new
baseline. Refreshing evidence changes package hashes, so regenerate the manifest
with `python3 make-manifest.py` and reverify it before redistributing a refreshed
package.

## Submit through existing SORA governance

1. Decode and submit the complete call in `preimage-note-call.hex` using a funded
   authorized account. If filling the `preimage.notePreimage` form manually, its
   bytes argument is `set-code-call.hex`. Do not place the complete note call in
   that bytes argument. Confirm the stored preimage matches the proposal hash
   and byte length above.
2. A council member submits
   `council-propose-external-majority-threshold-4-call.hex`. At the captured
   preflight there were eight members and the runtime required at least half
   for `democracy.externalProposeMajority`, so this call uses threshold four.
   Confirm membership and the origin requirement are still current.
3. Obtain the actual motion index and proposal hash from `Council.Proposed`.
   Members, including the proposer, explicitly vote aye. Close the motion only
   after the required votes, using its actual index and current weight bounds.
   Confirm inner dispatch succeeded and `Democracy.NextExternal` contains this
   exact runtime proposal hash.
4. A technical committee member submits
   `technical-committee-fast-track-threshold-3-call.hex`. At preflight there
   were four members; more than half requires three. The enclosed fast-track
   uses the current **1,800-block** minimum voting period and zero enactment
   delay. Members explicitly vote and close using the actual motion index and
   current bounds. Confirm inner dispatch succeeded.
5. Obtain the actual referendum index from `Democracy.Started`. Follow the
   existing referendum process and confirm successful enactment of this exact
   preimage. Preparing a collective proposal does not itself enact the runtime.

The bare `democracy-external-propose-majority-call.hex` and
`democracy-fast-track-call.hex` files are the inner calls for review. They need
the corresponding collective origin; ordinary signed submission cannot supply
that origin. Existing motions and proposals are not removed by this package.

## Verify activation on our nodes

After enactment, verify runtime spec **133**, transaction version **131**, and
publication marker `runtime:migrations:val_staking_rewards_published = true`.
For retained completed eras, check `staking.erasValidatorReward` equals the
existing `xorFee.valStakingEraReward`; verify the migration preserves claim
entries and that the active era is still excluded until completion.

Reload standard Polkadot.js staking payouts and check an eligible unclaimed
nomination. Confirm an ordinary claim transfers VAL, emits both standard and
VAL reward events, and marks the page claimed without increasing XOR stake or
minting XOR rewards. Normal XOR transaction fees still apply. Check subsequent
era closure publishes its VAL budget automatically.
