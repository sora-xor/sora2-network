# SORA Nexus runtime 137 — council handoff

This package contains the verified compressed runtime Wasm and unsigned governance calls. The artifact is byte-for-byte identical to the candidate that passed the final production Wasm rehearsals. It was checked again with `subwasm info`; its compiled runtime reports **spec version 137, transaction version 133, state version 0**, and spec name `sora-substrate`.

The live chain was checked at finalized block **28,191,490** on **2026-10-25 at 14:13:43 UTC** through `wss://mof2.sora.org`. It still runs spec **134**, transaction version **131**, with the exact reviewed runtime code hash. The chain genesis hash is `0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5`.

## Artifact and proposal identity

| Field | Value |
| --- | --- |
| Runtime file | `sora-nexus-runtime-137.compact.compressed.wasm` |
| Compressed runtime bytes | `1959169` |
| Runtime SHA-256 | `535b6499c78769a62701520d0bdf17ef65f07319450ca69a5264e99410e1910f` |
| Runtime BLAKE2-256 | `0x1b5d5366f3f72542da23a6ee7558068bc117ab4cec22d219c79fca51e1c38622` |
| Runtime upgrade call | `system.setCode(code)` — checked call, index `0x0002` |
| Upgrade preimage bytes | `1959175` |
| Upgrade proposal hash, BLAKE2-256 of encoded call | `0x31d3bee4e348dc89b97d2fde74f3d49be846c4127b75d640f513b747066bc36c` |

The proposal hash covers the complete SCALE-encoded `system.setCode` call. The runtime file hash is a different value. `runtime.preimage.call` is that complete call in binary; `runtime.preimage.hex` contains the same bytes as a `0x`-prefixed string. Its length fits the current 4,194,304-byte preimage limit. Both live-metadata encoding and an independent Python encoding check are included in the evidence.

## Council parameters

First note the upgrade preimage using `preimage.notePreimage` with `bytes` set to the complete contents of `runtime.preimage.hex`. Alternatively, `note-preimage.hex` is the complete unsigned `preimage.notePreimage` call for a client that accepts encoded calls. Do not paste the raw Wasm into the preimage bytes field: that field must contain the encoded upgrade call. The checked finalized state had no existing status for this preimage and no pending external democracy proposal; recheck those state-dependent facts before submission.

For `council.propose`, use:

```json
{
  "threshold": 5,
  "proposal": {
    "democracy.externalProposeMajority": {
      "proposal": {
        "Lookup": {
          "hash": "0x31d3bee4e348dc89b97d2fde74f3d49be846c4127b75d640f513b747066bc36c",
          "len": 1959175
        }
      }
    }
  },
  "length_bound": 39
}
```

This JSON describes the fields to select in the governance UI; `council-propose.hex` is the actual encoded call. The threshold is the helper's strict-majority policy for the **8** current council members. Recalculate if membership changes. The council origin itself accepts at least half; using 5 is valid and conservative. `length_bound=39` is the length of the inner democracy proposal, not the runtime or preimage length.

For council voting/closing, the council motion hash is `0xaa82445c0b9eaf2d7529872b76c1e8a4f3ccc388c2ee551717a6b11297c60137`. Use the motion index assigned by the chain after proposal submission and a closing weight bound obtained from current chain tooling. No motion index or closing weight has been invented for this package.

## Optional technical committee fast-track

The current helper defaults, encoded in `technical-fast-track.hex`, are:

| Field | Value |
| --- | --- |
| Outer call | `technicalCommittee.propose` |
| Threshold | `3` of the current `4` members |
| Proposal | `democracy.fastTrack` |
| `proposal_hash` | `0x31d3bee4e348dc89b97d2fde74f3d49be846c4127b75d640f513b747066bc36c` |
| `voting_period` | `1800` blocks, approximately 3 hours at 6 seconds per block |
| `delay` | `0` blocks |
| Outer `length_bound` | `42` bytes |

The technical motion hash for exactly these parameters is `0x60cdf9a6d0dd62b424d86adc11a21dfe802c087a6a4c6e03c4f913cfc71e73c8`. These are optional timing parameters for review. A zero delay provides no additional enactment buffer after passage. Schedule activation only after the node rollout and current-state migration preflight are complete; changing the timing requires regenerating the technical motion bytes and hash.

Sign governance transactions using the chain's active metadata and runtime version: currently **134 / 131**. The candidate's **137 / 133** becomes active after enactment. Refresh client metadata at activation; `System.CodeUpdated` now includes the code hash.

## Verification and release scope

Full Clippy passed in all three configurations, all 24 production Wasm rehearsal stages passed, and 943 prior passing Rust test executions remain bound to the unchanged source. The build-verification issue found during review is fixed. `evidence/rehearsal-summary.json` records the full scope and limitations; native replay is scoped, not a complete chain-state proof.

The runtime includes the stable2609 SDK migration and actual async staking with the custom staking adapter. Legacy staking state and historical offence handling are preserved as reviewed. New-era slashing intentionally follows the documented upstream incremental model and first-reporter treatment; see the included migration and review reports for the economic changes.

The artifact was built from the reviewed working tree, which includes uncommitted changes. Git HEAD `7fe885f651e682bde75f77f1ecbbc542d202a7b6` is its base commit, not a commit containing the entire upgrade. The compiled-source aggregate is `dbcaa1324c3e6ec4b2114da9148d24398f00db115037e3e4378fe9daa056a94d`; the fresh source check found no change to the 4,806 bound source files. Build evidence and source-check results are included.

This is a council review/submission package, not evidence that operators have upgraded. Final Linux Docker images and node integration checks, operator rollout confirmation, and a refreshed state-dependent migration admission check remain required before runtime activation. Obsolete bridge entries have the same decoding behavior under the live and candidate runtimes; their separate quorum cleanup is not a new upgrade blocker.

No private keys were accessed, no transactions were signed or submitted, and nothing was deployed. Verify package file integrity with `shasum -a 256 -c SHA256SUMS` from this directory. The checksum file verifies consistency; authenticate the expected runtime/proposal hashes through the council's normal release channel.
