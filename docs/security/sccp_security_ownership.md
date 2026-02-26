# SCCP Security Ownership Map

## Sensitive Paths

- `runtime/src/lib.rs` (SCCP runtime config and weight wiring)
- `pallets/sccp/src/` (bridge state machine, proof verification, mint/burn safety)
- `misc/sccp-mcp/src/` (tool exposure and transaction submission surface)
- `misc/sccp-mcp/config.example.toml` (secure operational defaults)

## Review Expectations

- SCCP-sensitive paths are explicitly mapped in `.github/CODEOWNERS`.
- PR-level SCCP review checklist is standardized in `.github/pull_request_template.md`.
- All changes in sensitive paths require at least 2 reviewers.
- At least 1 reviewer must be familiar with SCCP threat model and finality assumptions.
- Any change that touches proof verification, bridge domain validation, or submission tools requires explicit security checklist sign-off in PR description.

## Security Checklist (PR)

- Did this change expand externally reachable functionality?
- Are default settings still fail-closed / least-privilege?
- Are new or modified tools covered by policy allow/deny behavior?
- Are runtime weights and benchmark assumptions still valid for changed extrinsics?
- Are tests updated for both success and expected-failure security paths?

## Ownership Operations

- Keep `.github/CODEOWNERS` aligned with SCCP-sensitive paths and current maintainers.
- Re-run ownership topology export (security ownership map) at least once per quarter.
- Revisit bus-factor hotspots from `docs/security/audits/sccp-2026-02-20/ownership-map-out/` after maintainership changes.
- If a sensitive path becomes single-maintainer owned, prioritize pairing and cross-training before the next release.
