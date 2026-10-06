# Release Notes

## 4.7.6 — 2026-04-08

### Runtime
- Integrated the Polkamarkt prediction market pallet with on-chain buyback handling, canonical stable asset management, creation/gov-bond fees, and Polkadot Plaza broadcast hooks so the network can host curated markets out of the box (`runtime/src/lib.rs`, `pallets/polkamarkt/*`).
- Hardened the bridge subsystem with queue caps and minimum peer requirements, wired in the new `leaf-provider` pallet, and exposed BridgeProxy RPC/runtime APIs that enumerate deployed apps and their supported assets for downstream relayers (`runtime/src/lib.rs`, `node/src/rpc.rs`, `pallets/bridge-*`).
- Refreshed all Polkamarkt weights and transactional flows, covering buy/sell paths, benchmarking helpers, and runtime weight plumbing to keep fee estimates in sync (`pallets/polkamarkt/src/weights.rs`, `runtime/src/weights/polkamarkt.rs`).

### Tooling & Dev Experience
- Added purpose-built extrinsic builders for benchmarking along with new weight templates so `frame-benchmarking-cli` can drive remark and asset transfer calls without ad‑hoc scripts (`node/src/benchmarking.rs`, `misc/*weight-template.hbs`).
- Overhauled the runtime-upgrade helper with a documented release checklist, remote try-runtime rehearsal flow, and standalone preimage generation so governance submissions capture the exact metadata reviewed (`misc/runtime_upgrade/*`).
- Enforced dependency hygiene in CI with the `check_stable_deps_no_rc.sh` gate and refreshed static analysis to keep the release build pinned to audited crates (`.github/scripts/check_stable_deps_no_rc.sh`, `.github/workflows/static_analysis.yml`).

### Chain Specs & Dependencies
- Regenerated every chain spec to bake in the Polkamarkt defaults, bridge code-substitutes, and telemetry updates bundled in this runtime (`node/chain_spec/src/bytes/*.json`).
- Pinned the workspace to the `polkadot-stable2512-3` Polkadot SDK tag and vendored the upstream Substrate crates so reproducible cargo builds target the exact release state (`Cargo.toml`, `vendor/*`).
