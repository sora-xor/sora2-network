# SORA 4.8.12 paid-migration validation

Candidate SHA-256: `98f152040b1f084b53f7c2e024c0357c03ca62a89c4b2fd26546a0e446da55e4` (3,086,444 bytes).
Source tree: `086a0444ac493f5e60896051fa4dc1fbb9f8e078`; reviewed base: `bb38216396835fae45de9c38104e4927a96eb36c`.
Fresh native evidence records **1,151 passed**, zero failed,
**3 existing ignored**, across eleven affected suites, including
**32** runtime fee-policy regressions.

## Source and native execution

`source.patch` reapplies to the reviewed base and reconstructs the runtime source
overlay used by native tests and builds. Manifests, locks, modified production
code and regression files are hashed. The removed runtime sponsorship helper
and old sponsorship test module are recorded with baseline hashes and checked
absent. Generated chain-spec blobs are separately bound after their feature
builds. No clean-commit build is claimed.

The native recorder requires these six regressions by exact name:

- `outgoing_approval_retains_validation_weight_before_and_at_quorum`
- `migration_success_delivers_val_and_keeps_xor_fee`
- `migration_settlement_failure_keeps_fee_and_rolls_back_claim`
- `migration_requires_xor_before_valid_claim_can_execute`
- `migration_invalid_proof_and_replay_pay_without_duplicate_val`
- `wrapped_migration_requires_xor_and_keeps_success_fee`

Migration tests use real outer and Iroha signatures, backed VAL transfers,
transaction-fee events and XOR balance/nonce assertions. Pallet tests check
`Pays::Yes` on successful single claims and multisig approvals. Existing report,
keeper, order, reward/preimage and bridge-capacity tests remain included.
Standard transaction-extension weight correction remains in place; successful
migration has no fee waiver. Original native/format/build/Clippy log bytes are
retained without rewriting. Mainnet, try-runtime and extended Clippy profiles
must all finish successfully.

## Exact candidate Wasm

The pinned public snapshot supplies deployed code, metadata and state. Local
Chopsticks installs only the candidate code before upgrade initialization and
a timestamp. Checks preserve staking claims/ledgers and native XOR issuance.
Report rehearsal covers all four report methods, paid duplicate/wrapped failures,
unsigned admission/application rejection and disabled unsigned submission APIs.
Independent event checks match actual fee events to balance changes.

Fee-policy rehearsal covers useful cancellation/refund and paid replay, empty
cancellation shapes, unauthorized Rewards, requested-preimage first/replay,
maintenance fees, bare maintenance rejection, invalid inbound proofs, zero-XOR
authenticated bridge operations and replay rejection. Synthetic bridge-capacity
cases verify fair 32/33 quota behavior, honest admission, full-queue current-quorum
cleanup, no payload execution, preserved canonical state/foreign markers and
rejected stale/duplicate votes. Block weight/size accounting is isolated and
recorded; fixtures do not claim every transaction fits a single block.

New paid-migration fixtures verify all three transaction-validation sources and
block application reject an unfunded valid claim without changing its claim,
VAL or nonce. A funded claim transfers 300 VAL from the registered legacy
`Generic("bridge", "main")` technical account and retains the XOR fee. Replay
also pays without a second transfer. A referral conflict after the transfer
retains the fee while rolling back VAL and claim state. All key/value overrides
and synthetic funding are recorded. The deterministic proof fixture and its
Rust generator are hashed: Iroha ed25519/SHA3 ownership verification executes
inside Wasm; only outer transaction/header signatures use the mocked host.
No public transaction is submitted. These are local behavioral proofs, not
execution of a real user's mainnet claim.

Final fee-policy counts: **57 signed** cases
(**15 paid**, **42 free**), with fee/balance/nonce checks
passing. `paidMigrationSuccessAndFailure` and
`zeroXorMigrationRejectsBeforeExecution` are mandatory verifier gates.

## Compatibility and chain specs

Deployed spec 132 comparison strictly preserves existing pallet/call/event/error
indices, SCALE shapes, storage, signed extensions and runtime APIs, allowing the
reviewed additions and runtime-version constant change. Host interfaces and
memory compatibility retain their existing executor limitations.

A separate comparison to the sealed `210511d9...` unreleased candidate permits
only the two sponsorship calls, three sponsorship events, three appended errors
and `FeeSponsorships` storage to disappear. It rejects index/name reuse and then
strictly compares all remaining ABI/constants, without additions. Negative
self-checks cover extra storage deletion, migrate-argument changes, extension
reordering, additions and constant changes. The legacy `migrate` call remains 0;
bridge cancellation call 18 and all capacity storage remain present. No deployed
sponsorship state is claimed or cleared by this revision.

The offline chain-spec helper verifies exactly the same sponsorship-only removal
against each prior feature runtime. Stage/bridge staging use
`build-wasm-binary,private-net,stage`; test adds `wip,reduced-pswap-reward-periods`.
None uses `runtime/test`. Spec 134/transaction 131, private-network Sudo, bridge
capacity ABI, pallet indices and other SCALE encodings/constants are checked.
Only `:code` changes; all remaining JSON bytes/values and LastRuntimeUpgrade stay
unchanged. The report binds source, helper/comparator, feature build commands/log
hashes, metadata and blob hashes. Feature binaries/metadata are not duplicated
in the ZIP. `verify-package.py --check-workspace-source` checks actual JSON/blob
bytes in the checkout as well as all source hashes and deleted-file absence.

## Governance, history and limits

Finalized preflight block: **27,910,230**.
Set-code proposal: `0x0688cda5b9ff2d440fa06b43cced3597a140dbecc3858d392208d53042e05a3d` (3,086,450 bytes).
All six unsigned review calls are decoded against checked deployed metadata and
matched to the candidate and guarded settings. The occupied external queue is
preserved. Refresh finalized state before governance use; repeat baseline-sensitive
checks if the deployed code changes.

The previous candidate's evidence is historical in `predecessor-context.json`.
The older 430 maintenance/common/inbound results retain their original provenance
in `historical-owned-tests-context.json`; they are not fresh execution of this
revision. Readiness inspection is bounded and is not operator attestation or
complete bridge-backlog coverage. Bridge weight allowances remain conservative
estimates over old benchmarks, not newly measured worst-case throughput.

The manifest verifier binds distributable files, current reports/scripts, source,
Wasm, original logs, native/lint/build evidence, metadata comparisons, chain specs,
governance payloads and generated documentation. It rejects stale hashes,
missing regressions, restored sponsorship files and document placeholders.
`finalize-docs.py` renders current values from reports. Production consensus
execution, operator key/funding rollout and governance enactment are outside
this local evidence.

Fresh feature-runtime regeneration gate: **passed**.
Fresh three-profile Clippy gate: **passed**.
