# SORA 4.8.10 — BABE configuration repair

This release repairs the discrepancy between mainnet's canonical secondary-plain
blocks and the secondary-VRF configuration advertised by the runtime. The runtime
version is **132**; transaction version remains **131** because the extrinsic
encoding has not changed.
Completed tests, exact artifact hashes, source provenance and validation limits
are recorded in [VALIDATION.md](VALIDATION.md).

The runtime now returns the active epoch's probability and slot mode through
`BabeApi.configuration()`. Its one-time mainnet migration schedules Plain using
BABE's normal configuration-change mechanism. It preserves any existing governance
plan and skips unfamiliar configurations. Other networks retain their existing
genesis configuration.

**Activation is not immediate at code enactment.** The first epoch boundary after
the migration announces Plain and sets the next configuration. The second makes
Plain active. Mainnet epochs are one hour, so this normally takes between one and
two hours after the migration. If migration initialization coincides with a
boundary, that boundary can perform the announcement.

The node source also adds `repair-babe-epoch-cache`, an explicit offline recovery
command for an already incompatible client epoch cache. It diagnoses by default;
applying requires a new backup file. It uses finalized canonical headers to prove
the correct slot mode, preserves legitimate configuration announcements, and
changes only matching cache configuration fields. It does not relax block
verification or modify blocks, account state, block weights, or signing keys.
The database is opened with its existing exact schema, bypassing SDK automatic
metadata migrations. A write guard rejects initialization, pruning, and unrelated
logical writes; only one specific BABE cache replacement is permitted after the
backup is flushed to durable storage. Normal engine lock/log activity may still
occur while opening an existing database.

## Activation checks

Before enactment, verify the live mainnet genesis, the existing spec-131 runtime,
canonical Plain block production, `c=(1,4)`, both epoch configs VRF, and no pending
configuration change. The migration deliberately does not override a different
state or a governance plan. Run package validation again against the actual
deployment checkpoint if those preconditions change.

After enactment, distinguish the block containing `System.set_code` from the next
block that executes the runtime migration. Check the repair marker and pending
Plain descriptor after migration, the first boundary's `NextConfigData(Plain)`,
and then both current and next epoch configs plus all three BABE APIs after the
second boundary. Existing clients with an incorrect cache may not import those
boundaries; use the recovery procedure below.

## Recover an affected node

Stop its validator process before opening its database with the recovery command.
Use its existing chain argument, base path and database backend; do not guess a
different path or backend. The command uses an in-memory keystore and starts no
networking or authoring.
For evidence, the offline client ignores runtime substitutions in its in-memory
chain-spec copy and reads the finalized on-chain Wasm. The original chain spec and
normal validator startup retain their historical substitutions. Explicit runtime
overrides are refused.

First inspect:

```sh
framenode repair-babe-epoch-cache --chain <existing-chain> --base-path <existing-base-path> --database <existing-backend>
```

If it reports the verified VRF-to-Plain cache discrepancy, apply with a new backup
path on durable storage:

```sh
framenode repair-babe-epoch-cache --chain <existing-chain> --base-path <existing-base-path> --database <existing-backend> --apply --backup <new-backup-file.json>
```

Retain the JSON report and backup. Inspect again to verify that no changes remain,
then restart with the original validator flags and keystore configuration. Confirm
that the node advances its best and finalized heads, imports canonical Plain
blocks without the secondary-slot error, and resumes authorship. A positive key
presence check alone does not establish usable signing.

The command refuses unknown cache schemas, inconsistent epoch data, ambiguous
forks, a best head beyond the finalized state's epoch, and missing canonical
headers. It requires an explicit `--database rocksdb` or `--database paritydb` and
refuses missing databases or engine layouts that need migration. These refusals
are not permission to edit auxiliary bytes manually.

For a header-pruned/state-synced database lacking the required evidence, wait until
the corrected runtime state is finalized **and both current and next epoch configs
are Plain**. Keep the old database intact, then synchronize a separate fresh base
path using `--sync warp` as a non-authoring full node, retaining the existing chain
and network configuration. Verify its finalized checkpoint and canonical imports
before switching it to validator mode with the existing keystore path and password
mechanism. Never run the old and replacement instances as authors concurrently.
An ordinary restart of the old database does not rebuild its epoch cache.

## Scope

The reproduced configuration defect is shared by the chain, but local keys,
connectivity, clock synchronization and finality can independently prevent a node
from authoring. Recovery success must be checked on each affected node.

This directory is a local release candidate. No governance transaction, node
restart, database repair or production deployment has been performed by preparing
these files. Build hashes and completed validation results are recorded with the
finished artifacts.

The unsigned `set-code-call.scale`, `set-code-call.hex`, and
`preimage-note-call.hex` encode only the tested candidate. They are preparation
artifacts, not signed extrinsics or a submitted governance proposal. Re-run the
live preflight before using them. After preparation, verify package integrity
offline with:

```sh
python3 runtime-upgrade-4.8.10/validation/verify-package.py
```
