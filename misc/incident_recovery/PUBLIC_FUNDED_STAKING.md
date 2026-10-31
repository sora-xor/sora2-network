# SORA Nexus public funded staking fixture

This opt-in diagnostic joins a positive checkpoint account substitution, two
native HASHI deposits, the mapped-account balance journal, and real signed
ordinary staking transactions after runtime 138 activates. It is **public test
key material and synthetic external evidence. Never deploy its artifacts.**
No production recovery CSV, private signing key, host, external transaction,
validator signing route, balance override, fake signature or fee override is used.

Build the existing `incident-recovery` utility with `bridge-fixture`. The normal
production release corridor rejects these already published council/replacement
keys. The signed schema remains `sora-nexus-public-fixture-signed-maintenance-v1`;
normal/default maintenance replay admission remains unchanged.

1. Select a real, positive XOR checkpoint account with no staking position for
   the disposable substitution. Selection alone does not establish victim
   authorization or release eligibility. Run
   `incident-recovery prepare-funded-staking-identity --old <public-account> --signer-index 0`.
   Save its `csv`, `proofs` and `council` as ordinary fixture files. The proof uses
   the existing domain/genesis/checkpoint-bound ownership verifier and the
   published replacement key. It proves possession of that test key only.
2. Run the existing **complete typed** inventory, semantic/control/economic
   compilation and `compose-storage-work` on the verified checkpoint with those
   files. Preserve its actual process/source/input receipt. Do not hand-edit its
   storage report or replace a failed complete inventory with a balance-only
   move. The funding generator additionally re-executes every base guard,
   static audit and terminal invariant, but does not independently certify the
   typed compiler's coverage from a serialized report.
3. Save a manifest with exactly:

   ```json
   {
     "schema": "sora-nexus-PUBLIC-FIXTURE-ONLY-funded-staking-input-v1",
     "fixtureOnly": true,
     "baseWork": {"file": "base-storage-work.json", "sha256": "<exact SHA256>"},
     "signerIndex": 0,
     "expectedEra": 0,
     "firstSignedStep": 0
   }
   ```

   Replace `expectedEra` with the actual expected planned era (at most the single
   deferred boundary beyond the captured era), and `firstSignedStep` with the
   zero-based first ordinary final-138 step. These are assertions; neither
   changes state. The generator rejects a missing/zero original XOR balance,
   a used replacement, an existing staking position, altered mapped balance or
   nonce, missing static account witness, invalid base guards, or an oversized complete
   report/work cohort. Reports are stream-hashed and parsed with a strict 512 MiB
   bound. Every base chunk/audit remains mandatory; up to the normal work limit
   minus setup and two deposits is supported.
4. Run `incident-recovery prepare-funded-staking-fixture --capture <capture> --manifest <manifest>`
   into a fresh retained JSON output. The exact-artifact runner owns this real
   invocation; source compilation and runtime execution are separate gates.
   Extract `input`, `work`, `fixtureHashiProvenance` and
   `fixtureSignedTransactions` without editing their contents.
5. Build fresh final138, hold135 and bootstrap134 from `work.policyScale` through
   the existing pinned public-fixture exporter/build corridor, and the matching
   original134 native recorder. The original base policy is evidence, not the
   final runtime policy. Recompute the exact signed-step index after the actual
   code-stage count is known if necessary; rerunning the generator with only
   the step/era assertion changed must produce **identical work/policy bytes**.
6. Construct the signed-maintenance manifest with `fixtureSignedTransactions`
   pointing to the exact emitted plan, `fixtureHashiPeers` using the existing
   public HASHI peer scheme and emitted provenance, and `fixtureHashi: true`
   only on the two committed incoming steps. Retain at least the first recovery
   child before the deposits; the signer checks the actual authored birth header,
   its lineage marker, spec/transaction version, nonce, call and runtime state.
   Both committed peer eras are born at the setup block immediately before
   deposit 1, after every base chunk. The fixed Init → base chunks → setup →
   deposit 1 → deposit 2 schedule is checked; inserted delay/stage blocks before
   these deposits are rejected. Birth hashes still come only from actual authored
   retained headers and must match System.BlockHash and the recovery lineage.
   Supply the exact original native recorder. All incoming audits precede the
   unchanged baseline static audits; `deriveNativeBalanceAudits: true` is
   mandatory and `nativeBalanceAuditIndices` must be nonempty.
7. Execute normal `rehearse-maintenance` with the pinned manifest/output. The
   signed schema internally requires fresh opaque incoming evidence. Do not add
   `--require-prior-incoming-evidence` (there is no prior repair policy here).
   Retain its actual process receipt, independent block imports and stopped
   snapshot restores. Native/Wasm comparison must cover both deposits.

Configuration derives from `register_bridge`,
`register_existing_sidechain_asset(XOR, Sidechain)`, and the native scoped mint
permission grant in an isolated checkpoint projection. Only their exact allowed
configuration writes become guarded setup work. Existing mintability,
denominator, asset precision, balances, issuance, fees and staking state remain
unchanged by setup. The two deposits use the native U256 event decoder on a new
**non-Ethereum** HASHI network, preserving the Ethereum XOR prohibition. Each
credits 1,000,000 local XOR through normal authenticated incoming execution.
The source transactions and synthetic deposit association are public fixtures;
no external receipt or finality is claimed.

The mapped account's existing full System.Account value remains the original
static baseline. IncomingAudit9 consumes and checks both actual receipt/native
companions before NativeBalanceAudit10 chains the original baseline through
both authenticated changes. Configuration witnesses, all unrelated static
witnesses and typed terminal invariants remain required. The explicit issuance
witness and terminal arithmetic increase by exactly the two committed mints;
issuance/custody are never rebased through the account journal.

The emitted ordinary block contains real signed `bond`, `bond_extra`, `unbond`
and `rebond` extrinsics. Exact fee quotes, spendable balance, nonce, before/after
ledger, expected planned-era unbond deadline, dispatch result, block root,
independent import and snapshot restore must all match. Public key ownership
and original-policy/final-state incoming evidence are checked in process.

This fixture does **not** clear `maintenanceEpochLivenessAndStaking`: heartbeat,
BABE configuration/epoch intervals, deferred session/election/rewards and
operator consensus evidence still need their full independent checks. It also
does not establish full/warp sync, external finality or production readiness.

The earlier sealed `public-signed-maintenance-candidate-v1/README.md` says seven
source paths; its seal correctly lists **six**. This is the follow-up correction;
that immutable candidate was not edited.


### Normal funded fixture council identity

`prepare-funded-staking-identity --capture <checkpoint> --old <owner> --signer-index 0`
returns the root-verified, typed checkpoint council separately from the disclosed
public signing identities. The complete composer must preserve that authentic
council. The funding generator additionally checks exact original/projected
Council.Members bytes and refuses mappings of a council member. Public replacement
proofs authorize the test owner's ordinary transactions only. The explicit signed
fixture rejects all council escape fields and raw escape envelopes; no council
approvals, governance mutation, repair or production authorization is inferred.
