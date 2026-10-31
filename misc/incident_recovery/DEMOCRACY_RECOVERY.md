# SORA Nexus Democracy recovery

`utils/incident-recovery/src/democracy.rs` compiles a reviewed checkpoint voting
graph into one guarded, atomic storage group. It is pinned to the verified
block 28,191,817 metadata. The deployed constants are `MaxVotes = 500` and
`VoteLockingPeriod = 432000` blocks. The caller must still verify the complete
checkpoint, signed replacement mapping, unused destinations and the complete
storage inventory; this adapter does not authorize a release by itself.

The exact review contains a nonzero raw evidence commitment and records with
`storage`, `key`, and `valueHash` (null for a proved absence). It covers every
`Democracy.VotingOf` and `Democracy.ReferendumInfoOf` row, every referenced but
absent referendum, and the mapped voting owners' original `Balances.Locks` and
`System.Account` records, including absences. Unknown or ongoing referenda,
noncanonical keys/values, missing delegation targets, inconsistent aggregate
weight, duplicate votes, overflow and extraneous or omitted evidence all fail.

For a mapped delegating owner, the compiler applies the native `try_undelegate`
state transition: subtract only that owner's conviction-weighted capital from
its immediate target, retain incoming delegation weight, change the owner to
direct voting with an empty vote list, and accumulate the prior lock. The lock
end is the greater of its existing end and the first recovery child plus the
native conviction period; its capital is the greater of the existing prior
capital and the delegated balance. Arithmetic must be exact; saturation is not
accepted as evidence. Delegation through an intermediate target does not forward
the intermediate account's incoming weight again.

An uncompromised owner's delegation remains authorized, with a substituted
target where applicable. Direct voting records retain every vote and prior lock
byte; finished referendum records remain unchanged. A missing referendum is
preserved as an exact absence, consistent with the native cleared/cancelled
referendum branch. It is never invented as an approval or used to release funds.

The existing `democrac` balance lock must cover a mapped owner's voting capital
with the native `All` withdrawal reason. A zero-capital account may have no lock.
The compiler does not modify, reduce, recreate or release any balance lock or
System account record. `Compilation::verify_backing_moves` requires each existing
record to be relocated exactly once, byte-for-byte, by the generic ownership
compiler or another reviewed adapter. Absent records must remain absent. Final
witnesses bind the entire voting graph, referenda, and relocated custody records.
The replacement owner can later grant a new delegation using the normal signed
runtime call.

All voting/target changes must fit one recovery chunk (128 edits and the common
encoded-byte limit). An oversized mapping fails for a separately reviewed
component partition; it is not silently split across inconsistent graph states.
No runtime policy hash is embedded in generated state, so this adapter does not
introduce a policy/code commitment cycle.

The captured checkpoint observation contains 551 voting rows (549 direct and two
delegating), 797 finished referenda and no ongoing referendum. Nine recorded
votes reference absent referendum records. Both existing delegation grants have
zero balance; tests also cover positive balances across all seven convictions,
incoming grants and retained native locks. These observations and fixture tests
are not signed substitution approval or a complete release rehearsal.

Democracy proposals/deposits, governance collectives and Scheduler tasks have
separate ownership/authorization reviews. In particular, preserving a finished
referendum does not authorize execution of an old scheduled Root call. Scheduler
bookkeeping cancellation must preserve historical preimages and their deposit
accounting without dispatching those calls or invoking inferred refunds.
