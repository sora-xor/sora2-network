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
python3 verify-package.py
node verify-governance-calls.cjs
```

Before signing, inspect finalized state again using `prepare-package.cjs` and
`prepare-governance-calls.cjs`. These helpers allow public read-only RPCs and
encode unsigned calls; they never sign or submit. They require the tested
mainnet genesis, exact deployed spec-132 baseline and absent publication marker.
The governance helper rejects an occupied external queue or candidate blacklist
entry. The preferred council motion also checks queue vacancy atomically when
it executes. If a precondition changes, review and repeat validation against
the new baseline. Refreshing evidence changes package hashes. Run the following
in order before redistributing a refreshed package:

```sh
node prepare-package.cjs
node prepare-governance-calls.cjs
python3 prepare-council-settings.py
node verify-governance-calls.cjs
python3 make-manifest.py
python3 verify-package.py
```

Use `--check-workspace-source` on the final Python verification when the package
is inside its matching repository; an extracted council ZIP does not include
the whole source checkout. Check the manifest before running any scripts.

## Submit through existing SORA governance

1. Decode and submit the complete call in `preimage-note-call.hex` using a funded
   authorized account. If filling the `preimage.notePreimage` form manually, its
   bytes argument is `set-code-call.hex`. Do not place the complete note call in
   that bytes argument. Confirm the stored preimage matches the proposal hash
   and byte length above.
   At the refreshed checkpoint the preimage was absent. A new unrequested
   preimage requires a hold of **10.318376666564814900 XOR**, plus transaction
   fees. This is calculated from the deployed runtime's storage price; request
   status can change the hold requirement. Obtain a fee and balance quote from
   the actual account before signing. The 3,055,449-byte preimage fits the
   pallet's 4,194,304-byte limit.
2. A council member submits
   `council-propose-guarded-external-majority-threshold-4-call.hex`. At the captured
   preflight there were eight members and the runtime required at least half
   for both `democracy.externalPropose` and `democracy.externalProposeMajority`,
   so this call uses threshold four. Its inner call is
   `utility.batchAll([externalPropose(candidate), externalProposeMajority(candidate)])`.
   It fails atomically if another proposal occupies the queue at execution and
   otherwise queues this candidate with a simple-majority referendum threshold.
   The council `propose` length bound is **81**. Confirm current membership.
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

The direct `council-propose-external-majority-threshold-4-call.hex` and bare
`democracy-external-propose-majority-call.hex` are retained as **review-only
supersession alternatives**. They overwrite whatever occupies the external
queue at execution and bypass its blacklist check. Do not use them for this
preferred route. Any deliberate supersession requires a separate explicit
council decision. The bare utility and democracy calls need the corresponding
collective origin; an ordinary signed account cannot provide it.

## Vote and close settings

These are from finalized block **27,834,839**, checked **30 September 2026,
10:30 JST**. The chain still ran the exact tested spec-132 baseline; the external
queue, candidate blacklist and both collectives' motion lists were empty.

| Setting | Council | Technical committee |
| --- | --- | --- |
| Required explicit ayes | 4 of 8 | 3 of 4 |
| `propose.length_bound` | 81 | 42 |
| `close.length_bound` | 85 | 46 |
| `close.proposal_weight_bound.ref_time` | 325460526 | 595041000 |
| `close.proposal_weight_bound.proof_size` | 10700 | 3518 |

The guarded council **motion** hash is
`0x44d4bd9f8ea23612bfa609b09445b724e3f7d4504c4b51fa80b84453b0fd09ed`.
The technical committee **motion** hash is
`0x677291b7185f43a3f52864d8569a3a020f7437f3178ff4e008fc916e88a91ee0`.
Use those inner-call hashes for collective `vote` and `close`, together with
the actual index emitted by `Proposed`; do not use the runtime preimage hash
or the outer `propose` call hash. No motion index is preassigned by this package.

Close length bounds include the pinned SDK's documented four-byte storage-read
allowance. Weights were queried read-only from the deployed runtime for each
exact inner call. Requery the actual stored proposal under the live runtime
before closing, and confirm sufficient ayes and `Executed(Ok)`.
The technical fast-track uses **1,800 blocks** (nominally three hours at six
seconds per block), enactment delay **0**. A successful referendum and scheduler
enactment are still required. Machine-readable settings are in
`council-settings.json`.

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
