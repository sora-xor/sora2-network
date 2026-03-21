# SCCP Hub E2E Harness

This directory provides an SCCP matrix harness for local integration testing across:

- `sora2-network`
- `sccp/chains/eth`
- `sccp/chains/bsc`
- `sccp/chains/tron`
- `sccp/chains/sol`
- `sccp/chains/ton`
- `sccp/tools`

## What It Runs

The harness builds an ordered source/destination matrix over SCCP domains.

- Full matrix (`--matrix full`): 56 scenarios (`8 * 7`)
- SORA pairs (`--matrix sora-pairs`): 14 scenarios (`sora<->all domains`)
- SORA core pairs (`--matrix sora-core-pairs`): 10 scenarios (`sora<->ETH/BSC/SOL/TON/TRON`)

For each scenario, the harness executes these steps:

1. Source burn check (`sora` command or domain adapter/fallback)
2. SORA attest/mint step (when applicable)
3. Destination proof-toolchain check (non-SORA destination; backed by `sccp/tools/sccp-proof.sh` by default)
4. Destination mint verification
5. Negative verification step

## Adapter Contract

If an SCCP chain directory exposes `scripts/sccp_e2e_adapter.sh`, it is preferred.

Interface:

- `scripts/sccp_e2e_adapter.sh burn --json '<payload>'`
- `scripts/sccp_e2e_adapter.sh mint_verify --json '<payload>'`
- `scripts/sccp_e2e_adapter.sh negative_verify --json '<payload>'`

Payload example:

```json
{
  "scenario_id": "01-eth-to-sol",
  "source_domain": 1,
  "dest_domain": 3,
  "source_label": "eth(1)",
  "dest_label": "sol(3)"
}
```

If adapter scripts are missing, the harness falls back to commands in `config.local.json`.
Use `--strict-adapters` to fail when adapters are missing.

Notes:
- Core SCCP adapters (`sccp-eth`, `sccp-bsc`, `sccp-tron`, `sccp-sol`, `sccp-ton`) are required.
- `sora2-parachain` adapter install is optional; it is used for `sora_kusama` / `sora_polkadot`
  scenarios in full-matrix runs.

## Usage

Run full matrix:

```bash
misc/sccp-e2e/run_hub_matrix.sh
```

Dry run (no command execution):

```bash
misc/sccp-e2e/run_hub_matrix.sh --dry-run --skip-preflight
```

Single scenario:

```bash
misc/sccp-e2e/run_hub_matrix.sh --scenario eth:sol --skip-preflight
```

Strict adapter mode:

```bash
misc/sccp-e2e/run_hub_matrix.sh --strict-adapters
```

Disable cross-scenario command cache (release-grade independence):

```bash
misc/sccp-e2e/run_hub_matrix.sh --disable-command-cache
```

Run with a config mode preset:

```bash
misc/sccp-e2e/run_hub_matrix.sh --config misc/sccp-e2e/config.ci.json --mode release
```

## Artifacts

Each run writes to:

- `misc/sccp-e2e/artifacts/hub-matrix-<timestamp>/report.json`
- `misc/sccp-e2e/artifacts/hub-matrix-<timestamp>/junit.xml`
- step logs under the same run directory

`report.json` includes per-scenario step status, executed command, cwd, and log path.
Failed scenarios are classified with:

- `SOURCE_BURN_FAILED`
- `SORA_ATTEST_OR_MINT_FAILED`
- `DEST_PROOF_BUILD_FAILED`
- `DEST_MINT_FAILED`
- `INVARIANT_FAILED`
- `BUDGET_EXCEEDED`

## Configuration

Edit `misc/sccp-e2e/config.local.json` to tune:

- timeouts and max run time
- preflight command (`misc/sccp/run_all_tests.sh` by default)
- SORA step commands
- destination proof-toolchain command
- per-domain fallback commands
- matrix presets (`matrixPresets`)
- mode presets (`modes`) for:
  - mode-specific `defaults.maxMinutes`
  - mode-specific `defaults.commandCache`
  - mode-specific `commands.preflight.enabled`
  - default matrix selection (`mode.matrix`)

The canonical config key is `destinationProofToolchain`. The default backend now points at
the in-repo `sccp/tools` directory.

CI layout config:

- `misc/sccp-e2e/config.ci.json` expects the in-repo SCCP layout under
  `sora2-network/sccp/chains/*` and keeps preflight disabled by default.
- `misc/sccp-e2e/config.release-shadow.json` is the release-validation default
  used by `misc/sccp/verify_release.sh`; it is preflight-disabled and intended
  for prod-shadow confidence runs across the in-repo SCCP chains.

Local config modes:

- `local`: full matrix, preflight enabled
- `release`: full matrix, preflight disabled, command cache disabled
- `nightly`: full matrix, preflight disabled, command cache disabled

## CI Automation

- Workflow: `.github/workflows/sccp_hub_matrix.yml`
- Triggers:
  - manual `workflow_dispatch` with mode/matrix/negative/strict/scenario options
- Report summary script:
  - `misc/sccp-e2e/scripts/print_report_summary.sh <report.json>`

Tiered SCCP confidence gates:

- PR fast gate: `.github/workflows/sccp_confidence_pr.yml`
- Nightly exhaustive gate: `.github/workflows/sccp_confidence_nightly.yml`
- Release gate: `.github/workflows/sccp_confidence_release.yml`
