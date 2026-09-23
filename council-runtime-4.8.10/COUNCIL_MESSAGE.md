Subject: Council action requested — SORA mainnet runtime 4.8.10

Council members,

Please review the attached SORA mainnet 4.8.10 runtime upgrade. The current
runtime advertises BABE secondary-VRF slots while finalized canonical blocks
use secondary-plain slots. A validator that reconstructs its epoch cache from
runtime state can reject canonical blocks and fail to author even after coming
back online. The upgrade makes the BABE API report the active epoch correctly
and schedules Plain through BABE's normal epoch transition. It advances
runtime spec version 131 to 132; transaction version remains 131.

The exact tested Wasm is
`framenode-runtime-4.8.10.compact.compressed.wasm` (3,055,007 bytes, SHA-256
`a39de37798885ee253db508c553aca80321974fdd20fa0830bb94220cb9224c3`).
The preimage must be the attached full encoded `System.set_code` call, not the
raw Wasm: 3,055,013 bytes, BLAKE2-256 hash
`0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713`.
The merged source, tests and validation record are in
https://github.com/sora-xor/sora2-network/pull/1366 and `VALIDATION.md`.

A read-only finalized-chain check on 23 September found
`Democracy.NextExternal` holding hash
`0x3eaa14c5845c5de1234d7e8eea51ea0f7a9186803fe792028001d8bf8db1aa2c`
(length 3,036,855). Archived bytes identify it as a withdrawn 4.8.8 runtime
proposal for spec 130; its preimage was removed on 8 June. Mainnet already
runs spec 131. Please explicitly agree to retire this obsolete proposal. The
safer sequence in `GOVERNANCE.md` is: a Technical Committee member vetoes
that exact old hash while it is still queued; Council then approves the
guarded 4.8.10 motion, which fails atomically if another proposal has filled
the slot. An expired, unrelated Council motion #966 should also be closed as
disapproved while it remains at zero votes. Recheck all live conditions before
signing and verify the events after each step.

Register the full encoded 4.8.10 preimage first. Council approval only queues
the proposal; Technical Committee fast-track, a successful public referendum,
and enactment are separate steps. After enactment, Plain is normally announced
at the first epoch boundary and active at the second, about one to two hours
later. An affected validator's incompatible local BABE cache may still need
the documented offline repair or a fresh synchronization. No call in this
bundle has been signed or submitted.
