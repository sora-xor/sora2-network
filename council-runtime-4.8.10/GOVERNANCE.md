# SORA mainnet 4.8.10 governance runbook

These are **unsigned methods** for the exact tested Wasm in this directory.
They target genesis
`0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5`
and the deployed spec-131 metadata at the finalized block recorded in
`governance-calls.json`. Council and Technical Committee members use their
usual signing workflow. The route below clears the identified obsolete queue
item by its exact hash, then queues 4.8.10 with an atomic vacant-slot guard.

The preimage is the **full 3,055,013-byte encoded** `System.set_code` method in
`set-code-call.scale` or `set-code-call.hex`, with BLAKE2-256 hash
`0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713`.
The raw 3,055,007-byte Wasm is not the preimage.

## 1. Verify the package and current chain

From the extracted directory, run `sha256sum -c SHA256SUMS`. Confirm the
genesis above, deployed spec 131/transaction 131, on-chain runtime-code
SHA-256
`db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447`,
current and next BABE configs VRF with `c=(1,4)`, no pending BABE config
change, and finalized canonical secondary-plain blocks. Recheck that Council
has eight members and Technical Committee four. A changed runtime, membership
or BABE state requires new review and possibly new call encodings.

At packaging, `Democracy.NextExternal` held a withdrawn 4.8.8 runtime
proposal, lookup hash
`0x3eaa14c5845c5de1234d7e8eea51ea0f7a9186803fe792028001d8bf8db1aa2c`,
length 3,036,855. Its archived Wasm advertises spec 130, while current
mainnet is spec 131; the old preimage was cleared on 2026-06-08. Council and
Technical Committee should explicitly agree to retire this obsolete item.
If the queued hash is now different, stop and assess it before any veto or
Council close. The 4.8.10 target hash must not be blacklisted; it was not at
the packaging checkpoint.

## 2. Close unrelated stale Council motion; register the preimage

Council motion #966, hash
`0xcaa6b921f372c6d95087f4fa23fc73ed4b26ebeee8f03a3d64dfcf4b08835a57`,
was past its end block with zero ayes/nays at packaging. It concerns an
unrelated staking batch with an invalid declared preimage length. The
collective pallet still accepts votes after expiry.

If motion #966 is **still present, past its end, and has zero ayes and zero
nays**, any signed account may submit
`council-close-stale-motion-966-call.hex`. It encodes
`Council.close(hash, index=966, weight={0,0}, length_bound=0)`. Under that
checked state it disapproves and removes the motion. Confirm
`Council.Disapproved` and absent motion storage. If its votes, index or
metadata changed, do not use this packaged call. If already absent, skip it.
This cleanup does not clear `NextExternal`.

Submit `Preimage.note_preimage` with `preimage-note-call.hex`, or enter
`set-code-call.hex` as its bytes argument. Verify `Preimage.Noted` and that
the full bytes are available on-chain at
`{hash: 0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713,
len: 3055013}`. Obtain a fresh fee and deposit quote for the signing account.
Keep the preimage available through enactment.

## 3. Veto the exact obsolete queue entry

A **signed Technical Committee member** submits
`democracy-veto-obsolete-4.8.8-call.hex` only while `NextExternal` still
contains the exact old hash above. Pinned `Democracy.veto_external` checks the
queued hash before clearing it, so a changed proposal is not removed. Confirm
`Democracy.Vetoed` for that hash and that `NextExternal` is empty. This
blacklists the old hash for 28 days, although a future Council
`external_propose_majority` can bypass that blacklist. Recheck the member's
current fee and balance before signing. If the slot is already empty, skip
the veto; if it holds any different proposal, stop for fresh review.

## 4. Council proposes 4.8.10 with an atomic slot guard

A Council member submits
`council-propose-guarded-external-majority-threshold-4-call.hex`. It encodes:

| Field | Value |
| --- | --- |
| Outer call | `Council.propose` |
| Threshold | 4 of the checked 8 Council members |
| Inner call | `Utility.batch_all([Democracy.external_propose(target), Democracy.external_propose_majority(target)])` |
| Target | `Lookup{hash: 0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713, len: 3055013}` |
| Inner `length_bound` | 81 bytes |
| Inner Council motion hash | `0x96c58d0a985a47472653fbc4f7a9b06a9212cfd879c97da2b7f5ba4a72c90c3e` |

The first inner call requires `NextExternal` to be empty; if not, the
atomic batch fails and rolls back without replacing the new occupant. The
second changes the same target to a simple-majority referendum. The outer
method is 86 bytes and has a different hash. Use the **inner motion hash and
actual index** from `Council.Proposed`. Proposing casts no aye vote; at least
four members must vote aye.

Coordinate closing the motion only after the veto is finalized and
`NextExternal` is empty. Any signed account can close an approved motion.
If it is closed while the slot is occupied, it fails safely and is consumed;
Council would need to propose again. For a raw `Council.close` call, use the
actual proposal index, query `TransactionPaymentApi.query_info` for a current
weight bound, and use the raw stored proposal length plus four bytes for the
length bound. Confirm `Council.Executed` reports `Ok`,
`Utility.BatchCompleted` occurred, and `NextExternal` now identifies the
4.8.10 hash and 3,055,013-byte length. Outer extrinsic success alone is not
proof that the inner call succeeded.

## 5. Technical Committee fast-track and public referendum

After the Council motion succeeds, a Technical Committee member submits
`technical-committee-fast-track-threshold-3-call.hex`:

| Field | Value |
| --- | --- |
| Outer call | `TechnicalCommittee.propose` |
| Threshold | 3 of the checked 4 members |
| Inner call | `Democracy.fast_track(0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713, voting_period=1800 blocks, delay=0 blocks)` |
| Inner `length_bound` | 42 bytes |
| Inner Technical Committee motion hash | `0xb5363b379a987f3e9386671968b4a8a4e99ce0b37a2f9fbbb8701f1529dd0872` |

Proposing is not an aye vote. At least three members vote aye; close with
the actual emitted Technical Committee proposal index and fresh weight/length
bounds. Confirm `TechnicalCommittee.Executed` reports `Ok` and capture
the referendum index from `Democracy.Started`. The public referendum must
pass; fast-track does not enact the Wasm on its own.

After enactment, verify `System.CodeUpdated`, on-chain `:code` SHA-256 equal
to
`a39de37798885ee253db508c553aca80321974fdd20fa0830bb94220cb9224c3`,
spec version 132 and transaction version 131. The runtime migration runs
in the following block. Check its repair marker and pending Plain
descriptor; the first normal BABE epoch boundary announces Plain and the
second makes current and next Plain. This normally takes one to two hours.

The upgrade does not rewrite an affected node's incompatible local BABE
cache. Use the reviewed `repair-babe-epoch-cache` command or a separate
fresh synchronization as described in the merged 4.8.10
[README](https://github.com/sora-xor/sora2-network/blob/51949ad854fc7c1a04a2c6a0f1d352b292caac35/runtime-upgrade-4.8.10/README.md).
Confirm canonical imports, finality and authorship on each recovered validator.

## Council alternative: direct supersession

The file `council-propose-external-majority-threshold-4-call.hex` encodes a
direct Council motion with inner `Democracy.external_propose_majority(target)`,
length bound 39 bytes and inner motion hash
`0x993ca0695127d5c423ef9d65855fc5ca6789ea1d6de8c80392c67a0a09a36483`.
Council may choose this route to replace the obsolete 4.8.8 proposal without
a TC veto. It **overwrites whatever occupies `NextExternal` at execution**;
its call does not check for the exact old hash, and any signed account may
close it once approved. Council should explicitly approve that effect,
recheck the live slot before voting and closing, and verify
`Council.Executed(Ok)` plus the new queue entry afterward. Do not use the
guarded motion's hash or index for this distinct motion.

No call or transaction was signed or submitted while preparing this bundle.
