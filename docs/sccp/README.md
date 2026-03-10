# SCCP (SORA Cross-Chain Protocol)

SCCP is a burn/mint cross-chain protocol intended to be **fully on-chain**:

- burns create on-chain burn records + deterministic `messageId`
- mints require an on-chain verifiable proof that the burn `messageId` is finalized
- SORA governance only manages configuration and incident response; verification is intended to be light-client based
- token activation on SORA enforces deployed remote representations + endpoints on all SCCP core target domains (ETH/BSC/SOL/TON/TRON)
- SCCP required-domain config is canonicalized and validated at both governance update time and genesis build time (fail-fast on invalid genesis values); for first release it is pinned to the exact SCCP core domain set (ETH/BSC/SOL/TON/TRON)
- SCCP token registration is exclusive with legacy bridge routes: `add_token` rejects assets already on legacy bridges (EVM/TON), including queued legacy EVM `add_asset` requests
- inbound finality modes for ETH/SOL/TON are wired through pluggable on-chain verifier hooks:
  `EthFinalizedStateProvider`, `SolanaFinalizedBurnProofVerifier`, `TonFinalizedBurnProofVerifier`

## Docs In This Repo (SORA)

- `docs/sccp/FINALITY.md`: inbound-to-SORA finality definitions per source chain
- `docs/sccp/INBOUND_TOOLING.md`: how to generate and submit inbound proofs to SORA (EVM anchor + BSC/TRON light clients)
- `docs/sccp/HUB.md`: non-SORA -> non-SORA transfers via SORA on-chain attestation
- `docs/sccp/PROOF_TOOLING.md`: SORA -> destination proof generation (BEEFY+MMR) for destination verifiers
- `docs/sccp/EVM_ANCHOR_MODE.md`: governance-anchored EVM mode details
- `docs/sccp/BSC_LIGHT_CLIENT.md`: BSC header verifier details (inbound-to-SORA)
- `docs/sccp/TRON_LIGHT_CLIENT.md`: TRON header verifier details (inbound-to-SORA)
- `docs/sccp/RELEASE_CHECKLIST.md`: release-gate checklist and evidence package expectations
- `docs/security/sccp_mcp_deployment_guardrails.md`: MCP deployment hardening baseline
- `docs/security/sccp_security_ownership.md`: SCCP sensitive-path ownership and review policy

## Code In This Repo (SORA)

- `pallets/sccp/`: SCCP pallet (token registry, burns, mints, attestation, incident controls, BSC/TRON inbound verifiers)

## Sibling Repos (Destination Chains)

These repos implement SCCP routers/programs and **SORA BEEFY+MMR light-client verifiers** for minting on each chain:

- `../sccp-eth`
- `../sccp-bsc`
- `../sccp-tron`
- `../sccp-sol`
- `../sccp-ton`

## Tooling

Proof generation is implemented in:

- `../bridge-relayer` (`bridge-relayer sccp ...`)

MCP server for AI agents (stateless, external-signer-only):

- `misc/sccp-mcp` (`cargo run` in that directory; configure networks via `config.toml` and set `SCCP_MCP_AUTH_TOKEN` unless using inline `[auth].required_token`)

Coverage-guided proof-helper fuzzing:

- `pallets/sccp/fuzz` (`cargo fuzz run evm_proof_helpers`,
  `cargo fuzz run tron_proof_helpers`, `cargo fuzz run attester_quorum_helpers`,
  and `cargo fuzz run bsc_header_helpers`)

Cross-repo validation matrix:

- `misc/sccp/run_all_tests.sh` runs SORA pallet tests (`sccp`, `bridge-proxy`, `eth-bridge`),
  runtime SCCP integration tests (`framenode-runtime` `sccp_` subset), and sibling-chain tests
  (`../sccp-eth`, `../sccp-bsc`, `../sccp-tron`, `../sccp-sol`, `../sccp-ton`).
  The script requires those sibling repositories to exist one level above `sora2-network`.
  Override sibling root with `SCCP_DEV_DIR=/path/to/siblings` when CI layout differs.
  For Solana program test log verbosity, override `SOLANA_TEST_RUST_LOG`
  (default: `warn`).
  Solana program retries/parallelism are configurable via `SCCP_SOL_PROGRAM_RETRIES`
  (default: `2`) and `SCCP_SOL_PROGRAM_TEST_THREADS` (default: `1`).
  Retry pacing is configurable via `SCCP_SOL_PROGRAM_RETRY_DELAY_SECS` (default: `3`).
  Optional per-attempt timeout is configurable via `SCCP_SOL_PROGRAM_TIMEOUT_SECS`
  (default: `0`, disabled). If timeout is enabled, the script uses `timeout`
  (or `gtimeout` on macOS) when available.
  Solana program attempts are logged to `SCCP_SOL_PROGRAM_LOG_DIR`
  (default: `misc/sccp/logs` in `sora2-network`) with per-attempt files.
  Failure output includes a tail excerpt controlled by `SCCP_SOL_PROGRAM_LOG_TAIL_LINES`
  (default: `120`). Set `SCCP_SOL_PROGRAM_PRESERVE_LOGS=0` to delete successful attempt logs.
  Set `SCCP_SOL_PROGRAM_NOCAPTURE=1` to run Solana program tests with `--nocapture`
  for fuller per-attempt diagnostics.
  When a failure contains test-case markers, the script also prints a focused
  single-test rerun command for the first failed test.
  If `../sccp-sol/program` is temporarily blocking local progress, set
  `SCCP_SOL_PROGRAM_ALLOW_FAILURE=1` to continue the rest of the matrix while still
  reporting the failure. In CI mode (`CI=1` or `CI=true`), this override is rejected
  and the matrix remains fail-closed for `../sccp-sol/program`.
  `SCCP_SOL_PROGRAM_RETRIES` and `SCCP_SOL_PROGRAM_TEST_THREADS` must be positive integers;
  `SCCP_SOL_PROGRAM_RETRY_DELAY_SECS` and `SCCP_SOL_PROGRAM_TIMEOUT_SECS` must be non-negative integers;
  `SCCP_SOL_PROGRAM_LOG_TAIL_LINES` must be a positive integer;
  `SCCP_SOL_PROGRAM_ALLOW_FAILURE`, `SCCP_SOL_PROGRAM_PRESERVE_LOGS`, and `SCCP_SOL_PROGRAM_NOCAPTURE`
  accept only `0` or `1`.
  On retry exhaustion, the script prints an exact reproduce command for
  `../sccp-sol/program` with the effective test env.

Hub E2E matrix harness:

- `misc/sccp-e2e/run_hub_matrix.sh` orchestrates ordered SCCP source->destination
  scenario execution across `SORA/ETH/BSC/SOL/TON/TRON`, writes per-scenario logs,
  and exports both `report.json` and `junit.xml` artifacts under
  `misc/sccp-e2e/artifacts/hub-matrix-<timestamp>/`.
- It prefers sibling adapter scripts (`scripts/sccp_e2e_adapter.sh`) when present;
  otherwise it falls back to configurable per-domain commands from
  `misc/sccp-e2e/config.local.json`.
- Install/update sibling adapters from this repo via:
  `misc/sccp-e2e/install_sibling_adapters.sh`.
- For CI/workspace layouts where sibling repos are under `sora2-network/siblings`,
  use:
  `misc/sccp-e2e/install_sibling_adapters.sh --siblings-root "$PWD/siblings"`.
- Use `--dry-run` for command planning/validation without execution,
  and `--strict-adapters` to fail if adapters are missing.
- CI runner config is available at `misc/sccp-e2e/config.ci.json`;
  manual matrix automation is wired in `.github/workflows/sccp_hub_matrix.yml`,
  and scheduled exhaustive automation is wired in `.github/workflows/sccp_confidence_nightly.yml`.

Release verification orchestrator:

- `misc/sccp/verify_release.sh` executes the SCCP release evidence sequence:
  - `misc/sccp/run_all_tests.sh`
  - `misc/sccp-e2e/run_hub_matrix.sh --matrix full --strict-adapters --disable-command-cache`
  - bounded SCCP fuzz runs (`evm_proof_helpers`, `tron_proof_helpers`,
    `attester_quorum_helpers`, `bsc_header_helpers`)
  - bounded sibling fuzz runs (`../sccp-eth`, `../sccp-bsc`, `../sccp-tron`,
    `../sccp-ton`, `../sccp-sol`)
  - formal-assisted bounded checks (SORA + sibling `../sccp*` repos)
- Artifacts are emitted under:
  - `misc/sccp/artifacts/<timestamp>/summary.json`
  - `misc/sccp/artifacts/<timestamp>/junit.xml`
  - per-stage logs + hub-matrix report bundle.

PR fast verification orchestrator:

- `misc/sccp/verify_pr_fast.sh` executes PR-fast parity checks and emits artifacts under
  `misc/sccp/artifacts/pr-fast/<timestamp>/`:
  - SCCP-critical Rust tests (`sccp`, `bridge-proxy`, `eth-bridge`, `framenode-runtime` `sccp_`)
  - formal-assisted fast profile with sibling checks disabled
  - sibling smoke matrix scenario (`sora:eth`, no negative check)
- It auto-selects smoke config/mode by layout:
  - CI layout (`sora2-network/siblings/*`) -> `config.ci.json` + `mode=pr`
  - local sibling layout (`../sccp-*`) -> `config.local.json` + `mode=local`
- `misc/sccp/print_latest_evidence.sh` prints the newest release, PR-fast, and hub-matrix
  artifact statuses from local artifact directories. Hub status is sourced from the latest
  release artifact bundle when available.

Formal-assisted runner:

- `misc/sccp/run_formal_assisted.sh` includes sibling formal-assisted checks by default
  (`SCCP_FORMAL_INCLUDE_SIBLINGS=1`), delegating to
  `misc/sccp/run_formal_assisted_siblings.sh`.
- For local-only or PR-fast use, disable sibling checks with either
  `--exclude-siblings` or `SCCP_FORMAL_INCLUDE_SIBLINGS=0`.

Tiered SCCP CI gates:

- PR fast gate: `.github/workflows/sccp_confidence_pr.yml`
- Nightly exhaustive gate: `.github/workflows/sccp_confidence_nightly.yml`
- Release gate: `.github/workflows/sccp_confidence_release.yml`

Node runtime for EVM/TON sibling tests in CI:

- SCCP CI workflows pin Node.js `22.x` for `sccp-eth`, `sccp-bsc`, `sccp-tron`, `sccp-ton`.

Solana program flake stress loop:

- `misc/sccp/stress_sccp_sol_program.sh` repeatedly executes `../sccp-sol/program`
  tests and writes a run summary plus failure logs under `misc/sccp/logs/stress`.
  The summary includes failed iteration ids and parsed failure details
  (first failed test name + signature line) when failures occur.
  Useful env controls:
  - `SCCP_SOL_STRESS_RUNS` (default: `20`)
  - `SCCP_SOL_STRESS_TEST_THREADS` (default: `1`)
  - `SCCP_SOL_STRESS_DELAY_SECS` (default: `2`)
  - `SCCP_SOL_STRESS_TIMEOUT_SECS` (default: `0`, disabled)
  - `SCCP_SOL_STRESS_TEST_FILTER` (optional `cargo test` filter)
  - `SCCP_SOL_STRESS_NOCAPTURE` (`0|1`)
  - `SCCP_SOL_STRESS_STOP_ON_FAILURE` (`0|1`)
  - `SCCP_SOL_STRESS_ALLOW_FAILURE` (`0|1`)
  - `SCCP_SOL_STRESS_LOG_DIR` (default: `misc/sccp/logs/stress`)
  - `SCCP_SOL_STRESS_LOG_TAIL_LINES` (default: `120`)
  - `SCCP_SOL_STRESS_PRESERVE_PASS_LOGS` (`0|1`)
  On failures, the script prints a tail excerpt and a focused single-test rerun command.
  Latest local post-fix runs (2026-02-16) completed with:
  - `20/20` pass in hotspot-filtered and unfiltered 20-iteration stress loops.
  - `100/100` pass in hotspot-filtered stress loop.
  - strict full-matrix soak passing `3/3` consecutive runs.
  - post-expansion strict matrix rerun passed with runtime SCCP subset at
    `198 passed`, `0 failed` (`cargo test -p framenode-runtime sccp_ -- --nocapture`);
  Solana program attempt log:
    `misc/sccp/logs/sccp-sol-program.20260216-095303.attempt-1.log`.
