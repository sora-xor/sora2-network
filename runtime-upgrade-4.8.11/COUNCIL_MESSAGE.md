Subject: Council review requested — SORA mainnet runtime 4.8.11

Council members,

Please review the attached 4.8.11 runtime upgrade, advancing runtime spec
132 to 133 while retaining transaction version 131. It restores staking reward
discovery in standard clients by exposing recorded VAL budgets through standard
staking storage. XOR remains the staked asset and rewards remain VAL; the
upgrade disables native XOR reward minting and compounding on this path.

The bounded one-time migration publishes existing completed-era rewards inside
the 84-era claim window and preserves claim markers. Future eras publish their
VAL budgets automatically. Standard clients may still label displayed reward
amounts XOR; actual payments are VAL.

Use `framenode-runtime-4.8.11.compact.compressed.wasm`:

- Size: **3,055,443 bytes**.
- SHA-256: `15310a2f7899d458c84e47278c78f075fec74e3d6ab3de1fdc20ef914b14960a`.
- Full `system.setCode` preimage size: **3,055,449 bytes**.
- Preimage BLAKE2-256 hash: `0xc1d1b2e68fae6166a911f595ca2abc157afc97a12b14221191f9f6bfba6a9348`.

Register `set-code-call.hex` as the preimage bytes, or decode the complete
`preimage-note-call.hex` as the extrinsic call. Then use the preferred guarded
council motion in `council-propose-guarded-external-majority-threshold-4-call.hex`.
It requires **4 of 8** current council members and fails if another external
proposal occupies the queue at execution. Its proposal length bound is **81**.

After council execution succeeds and the exact hash is queued, use
`technical-committee-fast-track-threshold-3-call.hex`, requiring **3 of 4**
current technical committee members. Its proposal length bound is **42**,
voting period **1,800 blocks**, and enactment delay **0 blocks**. Members must
cast explicit votes, including proposers. Council and committee votes do not
replace the subsequent public referendum.

See `GOVERNANCE-RUNBOOK.md` and `council-settings.json` for the full sequence,
separate motion hashes and close settings. Obtain actual motion/referendum
indices from chain events and recheck finalized state before signing.

The exact candidate has recorded passing runtime/staking tests, upgrade and
payout rehearsals, and compatibility checks. Current source equivalence and
unsigned calls have been checked again. Review `REVIEW-4.8.10-to-4.8.11.md`,
`VALIDATION.md` and `SHA256SUMS` with the package. No transaction has been
signed or submitted by this preparation.
