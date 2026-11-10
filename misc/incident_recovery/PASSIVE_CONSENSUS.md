# SORA Nexus passive consensus verification

This corridor verifies exact local full/warp clients and deliberately kills and
restarts passive clients after observed committed/finalized recovery blocks.
It does not author blocks, use consensus private keys, connect to public peers,
publish, approve operators, or establish production readiness. No execution is
implied by the presence of this source.

The fixed entry point is `passive_consensus.py --manifest PATH --output NEW_PATH`.
The only admitted transport is `native-loopback`. The output must be outside the
source and input trees. Docker/remote routes are unsupported and reject without
fallback. Each long process has a 24-hour deadline. A failed attempt retains its
receipts; a timeout kill is never classified as a deliberate crash trial.

The manifest has schema `sora-nexus-passive-consensus-input-v1` and exactly these
fields: `schema`, `route`, `node`, `chainSpec`, `recoveryConfig`, `bootstrapWasm`,
`sourceInventory`, `predecessorDatabase`, `providerDatabase`, `target`,
`crashStages`, and `ports`. File entries contain a canonical relative `path` and
its `sha256`. The source inventory is the complete production-build inventory
of the current source, including these verifier files. The prepared spec must
have empty bootnodes/telemetry, the exact recovery extension and bootstrap bytes.
Known public fixture GRANDPA keys are rejected in active and approved lists.

Database entries contain a relative `path` naming only a quiescent RocksDB
directory and an `inventory` file entry. The inventory is an exact relative
filename to `{sha256, blake2_256, bytes}` map. Symlinks/hardlinks, credential paths,
keystores and network directories are rejected. Originals are only streamed and
copied; nodes never open them. Every lane receives an independent copy beneath
its owned chain database path. No stock database revert is performed.

The predecessor must end at block 28,191,816 with the real checkpoint parent
hash and no recovery-import marker. Its real GRANDPA set is checked when the
full client imports the checkpoint. The provider must end with coincident best
and finalized heads at the specified authentic recovery target, carrying a
complete real fresh-authority GRANDPA justification and retained state/history.
Prepare it externally from a retained valid database or the node's existing
offline signed-block import path. No synthetic BABE blocks or substituted
fixture authorities are accepted as production evidence.

A `target` contains `hash`, `number`, `stateRoot`, `codeHash`, `phase`, `cursor`,
and `operationCounter`. Each crash stage contains its own `providerDatabase`
and `target`; that database ends exactly at that target. Stage targets must be
on the final provider's freshly verified canonical recovery history. This
prevents races in which a provider advances past the intended crash block.
`ports` contains ten distinct explicit unprivileged local ports, alternating
RPC/P2P for three providers, one full consumer, and one warp consumer.

The controller preserves the SDK's three-peer warp threshold. Three provider
copies offer availability, not independent authority trust. All nodes use fresh
exact executable copies, passive roles, loopback listeners, reserved peers only,
empty discovery, disabled offchain workers/telemetry/mDNS, and BEEFY disabled.
Isolated starts generate owned libp2p network identities; those private identity
files are never read, copied to another lane, logged, or packaged. They are not
consensus keys. No host keystore is copied or used.

The optional node `--recovery-evidence-dir` writes bounded create-new receipts
for actual database opening, import attempts/results and successful SDK warp
verification. Raw accepted proof bytes are retained. No option preserves normal
node behavior. The stable2609 warp strategy imports proof headers before target
state (`network/sync/src/strategy/warp.rs`, Partial and Complete branches), so the
checkpoint header and recovery auxiliary marker must actually persist.

`verify-recovery-database` runs offline on a disposable copy of a stopped owned
database. It checks coincident exact best/finalized heads, checkpoint/lineage,
fixed-key storage proofs, original recovery status, code hash and consensus
auxiliaries. A generated persisted GRANDPA suffix is independently passed through
the unchanged recovery/SDK verifier. The actual warp process's accepted proof
bytes must also reverify to that same stored target. A trusted empty-signature
checkpoint marker alone cannot establish finality. Full verification additionally
requires every canonical recovery header and actual non-warp, executed import
receipts, including the clean checkpoint boundary.

Crash timing is explicitly **after an accepted import receipt and observed
committed/finalized target**. The recorded owned process group receives SIGKILL,
its actual signal exit is required, and a fresh exact executable reopens the
same owned database. Independent stopped-copy state/finality/auxiliary reports
must match before and after restart. These checks do not claim a kill inside a
RocksDB commit or malformed-chunk/escape/repair failure injection. The broader
`chunksCrashesAndRestarts` gate remains unchanged/unverified by this hook.

The optional release-package hook accepts `--passive-consensus-manifest`,
`--passive-consensus-source-sha256`, and `--passive-consensus-output` together.
The source binding is `passive_consensus_release.source_binding()`. It executes
the fixed corridor afresh, binds its node/spec/bootstrap/original policy/final
code to the package, and may verify only `fullAndWarpSynchronization`. It retains
explicitly inventoried public evidence files and committed-boundary crash
coverage. It never scans or packages network identities. It always preserves
`productionReady=false`.
