# SORA Nexus checkpoint recovery specification

The ordinary `main` chain specification is unchanged. Recovery uses a separate
specification with ID `sora-nexus-recovery-28191817`, protocol ID
`sora-nexus-recovery-28191817-v1`, and fork ID `recovery-28191817-v1`.

`framenode prepare-recovery-spec --recovery-config <reviewed-public-config.json>
--bootstrap-wasm <rehearsed-spec-134.wasm> --output <new-chain-spec.json>`
prepares that file offline. It copies the original raw genesis and historical
substitutes, clears inherited bootnodes and telemetry, and inserts the immutable
checkpoint substitute. An existing output file is never overwritten. Add only
approved recovery bootnodes after preparation; never reuse abandoned-branch
discovery endpoints.

The configuration schema is `RecoveryConfig` in `src/recovery.rs`. Hashes are
32-byte `0x` hexadecimal values; code commitments are BLAKE2-256 over the exact
Wasm bytes. Supply the real policy/data commitment, distinct bootstrap/final/hold
code hashes, reviewed evidence commitment, the pre-import client GRANDPA set ID,
ordered old and fresh authority/weight lists, and the independently reported
maximum abandoned BABE signing slot. Also supply sorted `retiredGrandpaKeys`
and `approvedGrandpaKeys` inventories covering the active, queued and registered
keys of every approved operator. The active authority lists are subsets of
these complete inventories; retired and approved keys must be disjoint.
The separate runtime `operatorsScale` binding contains each checkpoint owner,
its sorted retired GRANDPA key union, its fresh GRANDPA key, then sorted
`approved_babe` and `approved_im_online` key unions. Each retained-key union must
exactly cover that owner's active, queued and registered `NextKeys` values,
with one to three keys per type. Independent operator approval of those public
keys is required; the compiler cannot infer approval from checkpoint pointers.
The first child rejects missing, unused, duplicate or foreign-owner approvals
before changing consensus state. Final activation rechecks retained signing
keys after account replacement.
There are deliberately no production
defaults or example authority keys. A nonzero evidence commitment identifies
evidence; it does not itself validate that evidence. The release verifier must
validate its contents before producing an executable bundle.

Restore a separate database ending before the checkpoint and import the exact
checkpoint through 4.8.13. Authoring remains disabled until that exact import,
even with `--force-authoring`; retained headers can still be synchronized.
The importer records provenance atomically and checks
the old GRANDPA authority set before the forced S+1 transition. A database that
already contains the checkpoint without that provenance is rejected. Remove old
GRANDPA keys from the operator keystore, including pending keys; all remaining
GRANDPA keys must belong to the approved fresh inventory during maintenance.
Validators require an approved fresh key during that period. Retired keys remain
forbidden after activation; later ordinary session key rotations can use new
keys. This command never reads, generates, exports or removes private keys.

The same hard-fork descriptor reaches ordinary import and the SDK warp verifier.
The recovery warp wrapper explicitly inserts the trusted checkpoint transition,
because the stock proof generator stops at a forced change. Its boundary marker
has no signatures and is not presented as a finality proof. A genuine verified
post-checkpoint suffix is required before warp sync can finish. Each verifier
must cross the exact checkpoint in its authenticated proof sequence before any
recovery descendant; a lineage digest signed by old authorities cannot skip
that boundary. Split proofs retain this condition across verified segments.
The SDK imports intermediate proof headers, including the checkpoint, through
the same provenance-recording importer. Recovery headers
permanently carry the policy lineage digest; imported state must contain the same
policy commitment. Original finality proofs remain separate historical evidence.

The node refuses runtime overrides, duplicate substitute spec versions, malformed
recovery identities, reused GRANDPA keys, abandoned headers, wrong checkpoint
roots and stale first-child BABE slots. It does not bootstrap HASHI from existing
operator bridge files in recovery mode. It delays the offchain-worker runner
until recovery is complete, including during historical synchronization, and
then runs workers on subsequent imported blocks. Runtime bridge latches remain responsible
for preventing bridge operation after ordinary offchain workers resume.

Preparing a specification does not establish readiness. The final release needs
the approved account mapping and custody proofs, full checkpoint/state evidence,
settlement evidence, runtime proof adapters, immutable bootstrap/hold/final Wasm,
operator coordination, and full/warp/escape rehearsals against the exact bundle.

Node 4.8.13 refuses ordinary mainnet full startup, including RPC-only roles, unless the approved recovery extension is supplied. Explicit offline historical commands remain available. This guard runs after reading genesis and before bridge key bootstrap, networking, authoring or offchain workers. It prevents an unchanged operator command line from rejoining the abandoned branch.
