# SORA 4.8.11 staking reward validation

The packaged Wasm is **3,055,443 bytes**, SHA-256
`15310a2f7899d458c84e47278c78f075fec74e3d6ab3de1fdc20ef914b14960a`.
It advances spec **132 → 133**, retains transaction version **131**, and uses
impl version **1**. Exact-Wasm execution and unsigned-call checks below cover
this binary; native tests cover the recorded source commit.

The native macOS arm64 release build also passed and reports
`framenode 4.8.11-870df1dbe-aarch64-apple-darwin`. Its generated Wasm matches
the validated package byte for byte. Binary and build-log hashes are in
`validation/node-build.json`. This checks the local node build; Linux validator
binaries and production restarts have not been tested by this package.

The hosted Apps capture in `validation/hosted-client-evidence.json` shows that
its reward derive reads `staking.erasValidatorReward` and filters zero-budget
rows. The captured mainnet state reports zero there despite positive separate
VAL budgets. This is the defect corrected by the runtime candidate.

## Runtime and staking tests

The native release suites passed **200 runtime tests** and **218 staking tests**,
with no failures and four existing ignored runtime tests. New coverage checks
completed-era budget publication, bounded and idempotent migration, existing
claim preservation, direct and batched claims, rollback on failed VAL payment,
event order, and era closure without native reward issuance or XOR compounding.
Existing utility, multisig, reward destination and payment failure coverage runs
with nonzero standard reward budgets. Formatting passed.

Commands and log hashes are archived in `validation/native-tests.json` and
`validation/wasm-build.json`. An additional fail-closed native remote upgrade
rehearsal passed from a fresh live `wss://mof2.sora.org` scrape of the pinned
state, preserving a local snapshot outside the package. It used the filter in
`validation/native-remote-scope.json`; its command and outcome are in
`validation/native-remote-rehearsal.json` and the corresponding log.

## Exact Wasm execution

The candidate executed locally against public mainnet state pinned to block
**27,834,097**, hash
`0xf15e54191c639a904ef74a62fc4250178445e15221988069dba92c0523bb49c7`.
The deployed spec-132 Wasm was verified by SHA-256
`a39de37798885ee253db508c553aca80321974fdd20fa0830bb94220cb9224c3`.

- `Core_initialize_block` ran the actual upgrade migration. All **83** retained
  completed eras with positive VAL budgets became visible through standard
  staking storage. The migration made no claim or ledger writes and did not
  change XOR issuance.
- Unmodified `@polkadot/api-derive@16.5.6`
  `staking.stakerRewardsMultiEras`, with no custom SORA derives, discovered
  positive pending reward rows after migration.
- An ordinary `staking.payoutStakers` extrinsic for era **7706**, page **0**,
  paid **1.011945659813209697 VAL** to five recipients. Each recipient's XOR
  balance and ledger total, active balance and unlocking schedule were preserved.
  Standard `PayoutStarted` preceded successful `Rewarded` events.
- The fee-paying test burned **0.010014081258970732 XOR**, exactly accounting
  for the XOR issuance decrease. No native staking reward was minted.
- Both direct and by-page attempts to claim the same page again were rejected
  with `AlreadyClaimed`.

Full inputs, script hashes, dependencies, observations and RPC method counts
are in `validation/staking-reward-wasm-rehearsal.json`. The local executor uses
synthetic BABE headers and mocked signature verification; only `:code` is
manually replaced. This validates runtime migration and ordinary payout
execution, without proving real signatures, full block finalization, network
consensus or governance enactment. No remote transaction was submitted.

## Client and node compatibility

The structural comparator preserved existing SCALE encodings for **81 pallets,
405 calls, 442 events, 967 errors, 485 storage entries and 7 signed extensions**.
It found no metadata additions; only the runtime-version constant changed.
Runtime API versions, host function signatures and exports were preserved.

The declared initial Wasm memory grew from **45 to 46 pages**, an additional
**64 KiB**. The pinned SDK executor creates memory from the candidate's declared
minimum and adds its configured heap allocation, so the default allocation
supports the increase. This is explicitly recorded in
`validation/executor-memory-policy.json`; production operators' custom heap
settings were not inspected. Successful exact-Wasm execution supplements that
source review. See `validation/wasm-compatibility.json` and
`validation/metadata-compatibility.json` for the complete comparisons.

Public Apps can continue using its native XOR display label for the standard
reward field. Actual rewards are VAL. Generic exposure, paging and rounding
estimates can differ from dispatch results. Migration preserves the existing
84-era claim window and does not recreate expired entitlement.

## Source and unsigned package

The candidate was built from commit
`870df1dbe47507456faff6ab1975efc6493ce56d`, based on
`51949ad854fc7c1a04a2c6a0f1d352b292caac35`. The source patch was applied to a
clean base index and reproduced that commit's tree exactly. Source file hashes,
build command and persistent target directory are recorded in
`validation/source-provenance.json`. The generated Wasm lock contains 599
packages whose versions, sources and checksums match the repository lock, apart
from the generated runtime wrapper package. No fresh Cargo target was created.

Read-only package preflight at block **27,834,322** confirmed the expected
genesis, exact deployed spec-132 Wasm, absent publication marker and continuing
zero-standard/nonzero-VAL defect. Governance preflight at block **27,834,330**
found no external proposal or candidate blacklist entry. The unsigned
`system.setCode` call is **3,055,449 bytes**, proposal hash
`0xc1d1b2e68fae6166a911f595ca2abc157afc97a12b14221191f9f6bfba6a9348`.

The offline governance verifier decoded all six prepared calls using the exact
captured deployed metadata, checked the embedded Wasm and preimage byte for
byte, and checked collective thresholds and inner proposals. The global
package verifier checks the complete hash manifest, source patch and execution
reports. Re-run live preflight immediately before governance use. This package
has not enacted an upgrade or restarted any production node.
