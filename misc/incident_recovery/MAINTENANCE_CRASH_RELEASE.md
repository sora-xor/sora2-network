# SORA Nexus committed-snapshot process-death gate

The optional release-package hook runs a new `rehearse-maintenance-crashes`
process using the same explicitly pinned verifier and the exact manifest/input
closure already admitted by the package's fresh incoming and forward-repair
checks. It accepts no archived crash report, caller-supplied success flag, JSON
certificate, or public fixture council.

After integrating `maintenance-process-death-candidate-v2` into the utility and
building the exact verifier, obtain the host source binding with:

```sh
python3 misc/incident_recovery/maintenance_crash_release.py --print-source-binding
```

Add both options to the ordinary release-package invocation that supplies
`--evidence-verifier` and `--evidence-verifier-sha256`:

```text
--maintenance-crash-source-sha256 <printed exact source binding>
--maintenance-crash-output <new absolute local output directory>
```

No extra recovery manifest is accepted. The hook copies the existing complete
forward-repair input graph and invokes only:

```text
<owned exact verifier> rehearse-maintenance-crashes
  --manifest <owned exact forward-repair manifest>
  --output <owned execution/replay directory>
  --require-prior-incoming-evidence
```

The native parent performs the baseline afresh and retains its opaque incoming
certificate in process. The host rechecks all existing baseline block,
authorization, native-parity and historical-audit witnesses without constructing
or deserializing that certificate. Its diagnostic binding must equal both the
fresh native baseline and the already-fresh same-package incoming coverage.
The continued terminal snapshot must carry that exact final policy, root and
runtime 138 code.

For every baseline block, the worker imports the next child from the previous
killed worker's saved snapshot, commits and fsyncs the new snapshot, and emits a
barrier. The native parent authenticates that snapshot, issues SIGKILL to its
owned worker, observes signal 9, and independently restores the snapshot again.
A separate final worker restores the terminal snapshot and is also killed after
its barrier. The hook verifies every exact job, executable, block, engine,
launch, PID, barrier, exit signal, log hash, snapshot parent and receipt pin.
Reused PIDs are allowed across distinct completed process lifetimes.

Each worker has a 24-hour deadline. The fixed outer budget is
`(number_of_blocks + 3) * 24 hours`, leaving separate allowance for the fresh
baseline and terminal restoration. Failure/timeout retains evidence and never
registers partial package artifacts or clears the gate. Input/source/output
pins remain guarded through final atomic registration.

A verified `chunksCrashesAndRestarts` result has the exact scope
`committedSnapshotProcessDeath`, with `inCommitCrashVerified=false` and
`consensusProcessInterruptionVerified=false`. It covers process death after a
committed, fsynced maintenance snapshot and subsequent continuation. It does
not establish a crash during a RocksDB commit, consensus finality, operator
approval, source-build authenticity, or production readiness. The independent
`fullAndWarpSynchronization` gate remains required. The package's passive-node
interruption result is separate and cannot broaden this gate's scope.

Omitting both options preserves package behavior. Supplying only one fails
closed. Actual native compilation, runtime execution and process-death evidence
remain required; the Python suite contains synthetic receipts and mocked
processes only.
