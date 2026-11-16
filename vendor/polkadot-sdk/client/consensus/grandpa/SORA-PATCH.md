# SORA Nexus GRANDPA startup ordering

Copied from locked polkadot-stable2609 commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, package sc-consensus-grandpa 0.45.0.
`UPSTREAM-SHA256.json` records every original file. Existing copyright and license
notices are retained. Cargo.toml materializes upstream workspace package metadata,
lints and dependency settings; upstream SDK paths use the same stable2609 tag.

The scoped source change drains queued voter commands before polling any voter.
A recovery node delays voter construction until the checkpoint forced change is
installed. Its LinkHalf can still contain historical ChangeAuthorities/Pause
commands and an earlier in-memory round state. Polling a voter before consuming
those commands can sign using an intermediate set or round state.

Commands retain their original order and handler. Up to 64 commands are processed
per poll. If that budget is exhausted, the task wakes itself and services the
network while keeping voter polling paused. An endless command stream cannot
monopolize one executor poll or allow intermediate voting. Closed channels and
handler failures retain the original fatal behavior. Voting resumes only after
the queue reports Pending. Focused tests cover ordered draining, bounded backlog,
permanently ready queues and closed streams.

The node separately verifies checkpoint identity/import provenance and enactment
of forced set 7957 before constructing this voter. No signing keys, consensus
payload formats, authority transitions or historical proofs are changed here.

Warp state import also persists the new authority set and fresh voting rounds
in the same auxiliary write. Previously, the rounds were persisted only when
the queued ChangeAuthorities command ran, so a crash before it ran could reload
new authorities with stale rounds. A focused regression reloads the persisted
state before any voter command and checks set 7957, round zero and the new base.

While draining, historical commands still update the in-memory state in order,
but may persist their round state only if its set identity matches the current
shared authority set. That guard is held through the auxiliary write, excluding
a concurrent newer import. This prevents an intermediate old Pause/change from
overwriting the latest import's durable rounds before a crash. Current-set pause
and change persistence is preserved; the regression covers both cases.

The unchanged upstream communication tests also include the adjacent
`client/chain-spec/res/chain_spec.json` fixture. It is copied from the same
locked commit; its hash is recorded under `externalTestFixtures` so the
vendored package can run its test suite without an upstream source checkout.

The workspace Rust formatter also adjusts three line wraps in upstream
`communication/gossip.rs` and `tests.rs`; those changes have no semantic effect.
