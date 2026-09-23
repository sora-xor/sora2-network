# SORA 4.8.10 council handoff

Send `COUNCIL_MESSAGE.md` and this complete directory or ZIP to Council. The
Wasm is a byte-for-byte copy of the exact candidate rehearsed in merged
[PR #1366](https://github.com/sora-xor/sora2-network/pull/1366). The source
tree of that PR is now merged into main. A later build from another checkout
produced different Wasm bytes, so only the included candidate is covered by
the exact-Wasm rehearsal and call checks.

| File | Purpose |
| --- | --- |
| `framenode-runtime-4.8.10.compact.compressed.wasm` | Exact tested runtime candidate |
| `set-code-call.scale` / `set-code-call.hex` | Full SCALE-encoded `System.set_code` method; the preimage content |
| `preimage-note-call.hex` | Unsigned `Preimage.note_preimage` method |
| `council-close-stale-motion-966-call.hex` | Conditional disapproval of an unrelated expired Council motion |
| `democracy-veto-obsolete-4.8.8-call.hex` | TC member veto of the exact obsolete queued hash |
| `council-propose-guarded-external-majority-threshold-4-call.hex` | Preferred Council motion; refuses to replace a newly queued proposal |
| `council-propose-external-majority-threshold-4-call.hex` | Optional direct supersession route requiring explicit Council approval |
| `technical-committee-fast-track-threshold-3-call.hex` | Unsigned Technical Committee fast-track proposal |
| `governance-calls.json` | Encoded-call lengths, hashes and finalized metadata checkpoint |
| `GOVERNANCE.md` | Complete governance sequence, conditions and event checks |
| `VALIDATION.md` | Test results and limits from the reviewed release package |
| `SHA256SUMS` | File integrity hashes for this handoff |

The Wasm is 3,055,007 bytes with SHA-256
`a39de37798885ee253db508c553aca80321974fdd20fa0830bb94220cb9224c3`.
The `System.set_code` method is 3,055,013 bytes with BLAKE2-256 preimage hash
`0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713`.
These hashes identify different byte sequences and must not be interchanged.

## Live checkpoint and queue provenance

At finalized block **27,748,140**, hash
`0xd7226f7af356b1074acdd966f818f63bfb2912fede15010c4bc2bbd7ce8f6211`,
checked **2026-09-23 12:25 UTC** through `wss://mof2.sora.org` and
`https://mof2.sora.org`, the public chain reported:

- Genesis `0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5`.
- Spec version 131, transaction version 131, deployed `:code` SHA-256
  `db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447`.
- Current and next BABE epoch configs both VRF with `c=(1,4)`; no pending BABE
  change and no one-time repair marker.
- The last 20 finalized headers included secondary-plain pre-digests and no
  secondary-VRF pre-digests.
- Eight Council members and four Technical Committee members.
- `Democracy.NextExternal` occupied by hash
  `0x3eaa14c5845c5de1234d7e8eea51ea0f7a9186803fe792028001d8bf8db1aa2c`,
  length 3,036,855, simple-majority threshold.

The user-provided RPC `wss://ws.mof.sora.org` independently reported the same
genesis, spec 131 and queued lookup at finalized block 27,748,175. An archive
read at block **26,415,785** on 2026-06-03 recovered the queued preimage and
verified its BLAKE2-256 hash and 3,036,855-byte length. It is only
`Utility.batch_all([System.remark("Release 4.8.8"),
Utility.with_weight(System.set_code(...))])`. Its embedded Wasm advertises
spec **130**, transaction version **130**. The original proposer deliberately
removed that on-chain preimage with `Preimage.unnote_preimage` at block
**26,472,081** on 2026-06-08. A Council motion created in June executed on
2026-09-21 and queued the obsolete upgrade after mainnet was already at
spec 131. At the later finalized encoding checkpoint in
`governance-calls.json`, the same old hash still occupied `NextExternal`,
its blacklist entry was absent, and unrelated Council motion #966 still had
zero votes.

The recommended route is an exact-hash `Democracy.veto_external` signed by a
Technical Committee member, then an atomic guarded Council batch containing
`external_propose` and `external_propose_majority` for 4.8.10. The veto
fails if the queued hash changed. The guarded batch fails and rolls back if
anything else occupies `NextExternal`; when vacant, it queues 4.8.10 with a
simple-majority threshold. The separate expired Council motion #966 concerns
an unrelated staking batch with an invalid lookup length. Votes are still
possible after its end, so close it as disapproved while it remains at zero
votes. These steps and their exact state checks are in `GOVERNANCE.md`.

A direct Council `external_propose_majority` motion is also encoded as an
alternative. It overwrites **whatever** is in `NextExternal` when the motion
executes. It has no atomic exact-old-hash condition. Council must explicitly
choose that route after reviewing the live queued item; a signer could close
an approved Council motion before a planned last-minute recheck. The guarded
route avoids that replacement risk.

Run `sha256sum -c SHA256SUMS` from the extracted directory. Recheck finalized
chain state, membership, metadata, fees, deposit and motion votes before
signing. The previous 4.8.9 governance runbook describes the same
Council/Technical Committee/referendum flow, but its call hashes, thresholds
and indices are for another release and must not be reused.

No signed extrinsic, Council motion, referendum, runtime enactment or validator
database repair was performed in this handoff.
