## Summary

- Describe what changed and why.

## SCCP Sensitive Path Impact

- [ ] This PR touches one or more SCCP-sensitive paths:
  - `pallets/sccp/`
  - `runtime/src/lib.rs`
  - `runtime/src/tests/sccp_runtime_integration.rs`
  - `misc/sccp-mcp/`
  - `docs/security/`

If checked above:

- [ ] Requested at least 2 reviewers.
- [ ] At least 1 reviewer is familiar with SCCP threat model and finality assumptions.
- [ ] Completed the SCCP security checklist below.

## SCCP Security Checklist

- [ ] Does this change expand externally reachable functionality?
- [ ] Are defaults still fail-closed / least-privilege?
- [ ] Are new or modified tools covered by allow/deny policy behavior?
- [ ] Are runtime weights and benchmark assumptions still valid?
- [ ] Are tests updated for both success and expected-failure paths?

## Test Evidence

- List exact commands run and key pass/fail results.
