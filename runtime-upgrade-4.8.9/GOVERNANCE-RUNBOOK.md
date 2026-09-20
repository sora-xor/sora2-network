# Council and technical committee runbook

Use the files in this complete package. They target SORA mainnet genesis
`0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5` and deployed runtime 130. Checked at finalized
block 27624441 (`0x4540fa40a592c0f7f5fb925bed05d8507bb77048b1fd8647ec5235665f78d7e8`), 2026-09-12T12:12:21.973Z.
No calls have been signed or submitted by this task.

## 1. Verify the package and register the preimage

```sh
python3 validation/verify-package.py
```

Submit `Preimage.note_preimage(bytes)` using the full call in
`preimage-note-call.hex`. If entering the bytes argument manually, use
`set-code-call.hex` (or the binary equivalent `set-code-call.scale`). Do not use
the raw Wasm as the preimage argument: Democracy must execute the encoded
`System.set_code` call.

Verify the preimage is available on chain with:

- Hash: `0x6de23331a9c21a5ca06944b6cce28c0c0304c57ecd9b26af9fe2bb413b213b04`
- Length: `3055512`

The preimage is below the runtime's 4 MiB preimage limit. The submitter's signing
client should estimate the current fee and required preimage deposit.

## 2. Council proposes external majority

Submit `council-propose-external-majority-threshold-4-call.hex`. The alternative
threshold 5 file is supplied if the council elects to require five ayes.

Manual parameters:

| Field | Value |
| --- | --- |
| Call | `Council.propose` |
| threshold | `4` (minimum for the captured 8-member Council) |
| proposal | `Democracy.external_propose_majority` |
| proposal.proposal | `Lookup { hash: 0x6de23331a9c21a5ca06944b6cce28c0c0304c57ecd9b26af9fe2bb413b213b04, len: 3055512 }` |
| length_bound | `39` |
| Council motion hash (inner call hash) | `0xe3638e19ffec13e8924640beab8fd3cc1afd62285d79235fd5d4c939c402e015` |

**Proposing does not cast an aye vote.** Council members, including the proposer
if desired, must explicitly vote. Use the proposal index emitted by the actual
`Council.Proposed` event or the stored `Council.Voting` entry. Do not assume the
next index observed before submission will remain current.

After sufficient ayes, close the motion using that index, the Council motion
hash above, the dispatch-weight bound of the stored inner proposal, and its
stored length bound. The helper below prepares these unsigned vote/close calls
from the actual submitted motion. Observe `Council.Executed` and verify its
result is `Ok`; then confirm `Democracy.NextExternal` contains the runtime
proposal hash before proceeding.

Check `Democracy.NextExternal` before closing: `external_propose_majority`
replaces the pending external proposal. At the recorded validation block there
was no pending external proposal; avoid replacing an unrelated later proposal.

## 3. Technical committee proposes fast-track

After the council proposal executes, submit
`technical-committee-fast-track-threshold-3-call.hex`.

| Field | Value |
| --- | --- |
| Call | `TechnicalCommittee.propose` |
| threshold | `3` (strict majority of captured 4-member committee) |
| proposal | `Democracy.fast_track` |
| proposal.proposal_hash | `0x6de23331a9c21a5ca06944b6cce28c0c0304c57ecd9b26af9fe2bb413b213b04` |
| proposal.voting_period | `1800` blocks |
| proposal.delay | `0` blocks |
| length_bound | `42` |
| TechnicalCommittee motion hash (inner call hash) | `0xf11807988a87d489a1832a50ad9e2dbc3d81556424e6672638eda237d6ff9c31` |

Members explicitly vote aye; proposing is not a vote. Close when the threshold
is met, using the actual technical proposal index and computed weight/length
bounds. Verify `TechnicalCommittee.Executed` reports `Ok` and capture the
`Democracy.Started` referendum index.

## 4. Referendum and enactment

The public referendum must pass. Follow the normal referendum voting procedure,
using the actual emitted referendum index. With the prepared parameters, the
voting period is 1800 blocks and enactment delay is 0; enactment still follows the
runtime's referendum/scheduler processing. Do not treat committee approval alone
as proof that the upgrade has executed.

After enactment, check `System.CodeUpdated`, runtime spec/transaction 131, and the
on-chain code hash against this package. Observe bridge migration completion
(storage version 4 and cleared migration cursor) and the normal staking payout
flow. The separately included Apps patch needs its own frontend publication.

## Read-only helper for votes and closing

`validation/prepare-collective-actions.cjs` loads the stored council or technical
motion, discovers its actual index, and writes unsigned aye/close calls. It reads
runtime weight information and uses a conservative stored-length bound.
It never signs or submits a transaction. Run `--help` for the exact options.
Prepare these calls after the corresponding motion has appeared on chain.

The dependency manifest and lockfile used for the helper are provided in
`validation/rehearsal-package.json` and `validation/rehearsal-package-lock.json`.
From the extracted package directory, install those dependencies into a local
temporary directory and inspect the finalized council motion:

```sh
UPGRADE_JS_DEPS="$(mktemp -d)"
cp validation/rehearsal-package.json "$UPGRADE_JS_DEPS/package.json"
cp validation/rehearsal-package-lock.json "$UPGRADE_JS_DEPS/package-lock.json"
npm ci --ignore-scripts --prefix "$UPGRADE_JS_DEPS"
NODE_PATH="$UPGRADE_JS_DEPS/node_modules" node validation/prepare-collective-actions.cjs \
  --collective council --out "$UPGRADE_JS_DEPS/actions"
```

Run it again for the technical motion after that proposal is finalized:

```sh
NODE_PATH="$UPGRADE_JS_DEPS/node_modules" node validation/prepare-collective-actions.cjs \
  --collective technicalCommittee --out "$UPGRADE_JS_DEPS/actions"
```

The printed `outputDirectory` contains a report and, when the matching motion
exists, `council-aye.hex` / `council-close.hex` or
`technicalCommittee-aye.hex` / `technicalCommittee-close.hex`. These contain
encoded calls for the council's usual signing client. Each run creates a fresh
directory. A motion not yet submitted or already closed produces no vote/close
files, because its next proposal index cannot be predicted safely.

At finalized block 27624494 the inner council weight was
`{ refTime: 102198000, proofSize: 0 }` and technical weight was
`{ refTime: 595041000, proofSize: 3518 }`. These are evidence, not values to keep
reusing after runtime changes. The helper reads fresh weight using
`TransactionPaymentApi_query_info` on an unsigned inner proposal and sets close
length to the raw stored proposal length plus four bytes. The close bound is
for the small collective inner call, not the 3,055,512-byte Wasm preimage.

`validation/collective-actions-check.json` records the successful public RPC
check with zero submissions. Six offline tests verify actual stored-index
discovery, both call encodings and bounds, absent-proposal behavior, and
inadequate-threshold rejection using the captured metadata. They can be rerun:

```sh
NODE_PATH="$UPGRADE_JS_DEPS/node_modules" node --test validation/prepare-collective-actions.test.cjs
```

Recheck membership and runtime version if submission is delayed. A changed
collective membership can change thresholds; a different deployed runtime may
require regenerating the unsigned calls from its metadata.
