# SORA Nexus runtime 137 upgrade evidence

The council handoff is in [council/README.md](council/README.md). It contains the compressed runtime Wasm, preimage, unsigned governance calls, parameters, and file checksums. The runtime is spec **137**, transaction version **133**; the node package version remains **4.8.12**.

Start review with the [live-to-candidate review](../misc/runtime_upgrade/live_to_137_review_2026-10-25.md), [async staking migration](../misc/runtime_upgrade/async_staking_migration_2026-10-25.md), and [activation-readiness report](../misc/runtime_upgrade/bridge_activation_readiness_2026-10-25.md).

## Recorded verification

The [final summary](validation/rehearsal-summary.json) records three full Clippy configurations, all 24 production Wasm stages, and 943 retained passing Rust test executions with three existing ignored tests. The later build-verification fix reran the build, Clippy, and Wasm stages while retaining the original native-test and dependency-audit timestamps. The reports are historical evidence of those exact executions, not a claim that all checks were rerun while opening the PR.

The candidate SHA-256 is `535b6499c78769a62701520d0bdf17ef65f07319450ca69a5264e99410e1910f`. Its compiled runtime version and governance encoding were checked independently again for the council handoff.

The [PR source binding](validation/pr-source-binding.json) confirms all 4,806 recorded source inputs were present byte-for-byte, with their original executable modes, in implementation commit `ae9da41dd96775444fdcb0de35978ece26acd5c2`. That commit is based on current `master`; its parent tree is identical to the original tested base tree. A separate CI-only follow-up adds the async-staking audit graph and offline guard tests. Its scope and validation are recorded in [the CI follow-up evidence](validation/pr-ci-verification.json).

## Reproducing the evidence

Scripts, lockfiles, required fixtures, current reports, and candidate artifacts are included. Native `.snap` files, superseded attempt directories, the public RPC cache, and `bridge-request-recovery-pages.jsonl.gz` remain local. Complete aggregation requires restoring or recapturing those inputs; the checked-in report hashes do not replace the missing raw data. Public RPC state is trusted diagnostic input, not an independently verified consensus or whole-chain state proof.

The native suites and Wasm rehearsals have different scopes, documented in the final summary. They do not establish final Linux Docker compatibility, existing-node database restart behavior, mixed-version consensus, or operator readiness. Verify final node images and complete operator rollout before runtime activation, then refresh state-dependent migration admission checks.
