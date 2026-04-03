# SCCP Nexus-Hub E2E Harness

This harness keeps the existing `run_hub_matrix.sh` entrypoint, but its model is now:

- source spoke burn
- Nexus hub bundle publication in `../iroha`
- destination spoke mint verification
- optional negative verification

SORA2 is treated as a spoke, not as the SCCP hub.

## What It Runs

The harness still builds an ordered source/destination matrix over SCCP domains:

- `full`
- `sora-pairs`
- `sora-core-pairs`

For each scenario it executes:

1. Source burn check on the source chain
2. Nexus hub bundle publication/fetch check from the `hub` config block
3. Destination mint verification on the destination chain
4. Optional negative verification

If a chain directory exposes `scripts/sccp_e2e_adapter.sh`, that adapter is preferred. When no
adapter is available, the harness falls back to the commands in the active config JSON.

## Configuration

The active config files are:

- `misc/sccp-e2e/config.local.json`
- `misc/sccp-e2e/config.ci.json`
- `misc/sccp-e2e/config.release-shadow.json`

Important keys:

- `paths.sora2Network`: local SORA2 repo root
- `paths.iroha` or `paths.hub`: Nexus hub repo root
- `commands.sora`: SORA spoke checks (`burn`, `mint_verify`, `negative_verify`)
- `commands.hub.publish_bundle`: Nexus hub proof/bundle publication check
- `commands.domains.*`: per-domain burn / mint / negative checks

When `SCCP_MESSAGE_ID` is already present in the scenario context, the default config can fetch a real
bundle from Nexus Torii through `misc/sccp-e2e/src/fetch_nexus_bundle.js`. When that context is not
available, the hub step falls back to the in-repo `iroha_torii` bundle endpoint tests.

The fetched hub artifacts are stored as JSON plus Norito bytes. The harness exports
`SCCP_HUB_BUNDLE_NORITO_PATH` / `SCCP_HUB_BUNDLE_NORITO_HEX` as the canonical proof-byte inputs and
also mirrors them into the older `SCCP_HUB_BUNDLE_SCALE_*` names as a temporary compatibility bridge
for downstream scripts that have not been migrated yet.

The canonical hub command key is `commands.hub.publish_bundle`. Older proof-toolchain key names are
still accepted by the script as a compatibility fallback, but the configs in this repo now point at
`../iroha`.

## Usage

Run the default matrix:

```bash
misc/sccp-e2e/run_hub_matrix.sh
```

Dry run:

```bash
misc/sccp-e2e/run_hub_matrix.sh --dry-run --skip-preflight
```

Single scenario:

```bash
misc/sccp-e2e/run_hub_matrix.sh --scenario sora:eth --skip-preflight
```

Release-shadow config:

```bash
misc/sccp-e2e/run_hub_matrix.sh --config misc/sccp-e2e/config.release-shadow.json --mode release
```

## Artifacts

Each run writes:

- `misc/sccp-e2e/artifacts/hub-matrix-<timestamp>/report.json`
- `misc/sccp-e2e/artifacts/hub-matrix-<timestamp>/junit.xml`
- step logs under the same directory
- per-scenario `scenario-context.json` files with propagated `message_id`, payload, and hub bundle artifacts

Useful environment variables:

- `SCCP_NEXUS_TORII_URL`: override the Torii URL used by `fetch_nexus_bundle.js`

Failure codes include:

- `SOURCE_BURN_FAILED`
- `HUB_BUNDLE_PUBLICATION_FAILED`
- `DEST_MINT_FAILED`
- `INVARIANT_FAILED`
- `BUDGET_EXCEEDED`

## CI

The CI and release wrappers still invoke `misc/sccp-e2e/run_hub_matrix.sh`, but the hub repo they
exercise is Nexus in `../iroha`, not SORA2.
