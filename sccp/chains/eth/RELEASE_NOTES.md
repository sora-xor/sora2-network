# Release Notes

## SCCP Hardening Update

### Summary
- Stabilized the full fuzz stack (`fastcheck` + `foundry` + `echidna`) with deterministic precompile behavior.
- Added CI wrapper scripts for formal and fuzz checks with repository hygiene gates.
- Added deploy-target compile checks and wiring to prevent script drift.
- Hardened local runtime behavior for Hardhat by preferring Node 22 when available.
- Added workflow-level checksum verification for Echidna downloads in nightly fuzz CI.

### Key Reliability Changes
- Added/validated CI entrypoints:
  - `test:ci-formal`
  - `test:ci-fuzz`
  - `check:repo-hygiene`
- Added `compile:deploy` and ensured it compiles only production contracts.
- Added final hygiene verification at the end of the formal CI wrapper.
- Ensured Python cache artifacts do not leak into workspace checks.

### Security and Supply Chain
- Nightly fuzz workflow now verifies the pinned SHA-256 of the Echidna release tarball before installation.

### Validation Snapshot
- `npm run compile:deploy`: pass
- `npm run test:deploy-scripts`: pass
- `npm run check:repo-hygiene`: pass
- `npm run test:ci-formal`: pass
- `npm run test:ci-fuzz`: pass
- `npm run test:fuzz`: pass

### Notes
- Hardhat commands are now routed through a wrapper that can use Homebrew Node 22 (`node@22`) when the shell default is a non-LTS Node.
