# SCCP (SORA Cross-Chain Protocol)

SCCP in this repo is the SORA2 spoke runtime, not the SCCP hub.

- Sora Nexus mainnet in `../iroha` is the SCCP hub and proof source.
- This repo consumes Nexus burn bundles and Nexus parliament governance bundles.
- Local SCCP state is proof-driven only: `burn`, `mint_from_proof`, `add_token_from_proof`, `pause_token_from_proof`, and `resume_token_from_proof`.
- SORA2 no longer exposes local SCCP hub commitments, `attest_burn`, or local SCCP governance/incident-control flows for downstream chains.

## Docs In This Repo

- `docs/sccp/HUB.md`: current Nexus-hub / SORA2-spoke overview.
- `docs/security/sccp_mcp_deployment_guardrails.md`: MCP deployment hardening baseline.
- `docs/security/sccp_security_ownership.md`: SCCP sensitive-path ownership and review policy.

## Code In This Repo

- `pallets/sccp/`: SCCP spoke pallet.
- `utils/iroha-proof-runtime-interface/`: verifier boundary for Nexus finality proofs and Nexus parliament certificates.
- `runtime/src/lib.rs`: runtime wiring for Nexus proof verification.
- `node/src/service.rs`: node-side host implementation for the runtime proof interface.

## Proof Source

The live SCCP proof schemas, proof publication, and parliament-sourced governance certification now live in `../iroha`.
This repo only verifies and applies those Nexus proof bundles.
