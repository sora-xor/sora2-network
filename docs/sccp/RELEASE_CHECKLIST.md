# SCCP Release Checklist

This checklist defines the release-gate evidence package for SCCP-critical code.

## Preconditions

- `sora2-network` is checked out with the in-repo SCCP chain directories available:
  - `sccp/tools`
  - `sccp/chains/eth`
  - `sccp/chains/bsc`
  - `sccp/chains/tron`
  - `sccp/chains/sol`
  - `sccp/chains/ton`
  - `../sora2-parachain` (for `sora_kusama` / `sora_polkadot` matrix domains)
- Rust toolchain for `sora2-network` SCCP validation is `nightly-2025-05-08`.
- `sora2-parachain` SCCP adapter checks are pinned to `nightly-2023-03-21`
  (via `SCCP_PARACHAIN_RUSTUP_TOOLCHAIN`, default set in the adapter script).
- Node.js `22.x` is used for EVM/TON SCCP chain test execution.

## Required Command

Run the SCCP release gate orchestration:

```bash
misc/sccp/verify_release.sh
```

PR-fast parity evidence (recommended before release gate):

```bash
misc/sccp/verify_pr_fast.sh
```

PR-fast artifacts:

- `misc/sccp/artifacts/pr-fast/<timestamp>/summary.json`
- `misc/sccp/artifacts/pr-fast/<timestamp>/junit.xml`

Useful overrides:

```bash
SCCP_VERIFY_HUB_CONFIG=misc/sccp-e2e/config.ci.json \
SCCP_VERIFY_HUB_MODE=release \
SCCP_VERIFY_MATRIX_MODE=full \
SCCP_VERIFY_DISABLE_HUB_CACHE=1 \
misc/sccp/verify_release.sh
```

## Required Passing Stages

The release gate is blocking if any stage fails.

1. `run_all_tests`
2. `hub_matrix`
3. `fuzz_bounded`
4. `fuzz_bounded_siblings`
5. `formal_assisted` (includes in-repo `sccp/chains/*` formal-assisted checks)

## Required Artifacts

Each release validation run must produce:

- `misc/sccp/artifacts/<timestamp>/summary.json`
- `misc/sccp/artifacts/<timestamp>/junit.xml`
- stage logs under `misc/sccp/artifacts/<timestamp>/logs/`
- hub matrix evidence under `misc/sccp/artifacts/<timestamp>/hub-matrix/`
  - `report.json`
  - `junit.xml`

## CI Gates

- PR fast gate: `.github/workflows/sccp_confidence_pr.yml`
- Nightly exhaustive gate: `.github/workflows/sccp_confidence_nightly.yml`
- Release gate (required before tag): `.github/workflows/sccp_confidence_release.yml`
