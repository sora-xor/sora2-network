# SCCP-ETH Audit Report (2026-03-02)

## Scope
- Contracts: `SccpRouter`, `SccpToken`, `SccpCodec`, `ISccpVerifier`, `SoraBeefyLightClientVerifier`
- Supporting code: deployment scripts, fuzz harnesses, CI workflows, E2E adapter script
- Verification baseline:
  - `npm run compile`
  - `npm test`
  - `npm run test:formal-assisted`
  - `npm run test:fuzz:fastcheck`

## Summary
- Critical: 0
- High: 0
- Medium: 1
- Low: 2
- Informational: 1

## Findings

### F-01 (Medium) — CI supply-chain exposure in nightly fuzz workflow
- Location: `.github/workflows/sccp_fuzz_nightly.yml:20`, `.github/workflows/sccp_fuzz_nightly.yml:38`
- Evidence:
  - Uses mutable tag `actions/setup-node@v4`.
  - Uses `curl -L https://foundry.paradigm.xyz | bash` (remote script execution without digest pinning).
- Impact:
  - If upstream action tag or install script is compromised, attacker-controlled code can execute in CI.
- Remediation plan:
  1. Pin `actions/setup-node` to a full commit SHA.
  2. Replace `curl | bash` with a pinned release artifact install (version + SHA256 verification), or a pinned toolchain action SHA.
  3. Add periodic dependency refresh process with explicit review.

### F-02 (Low) — Mainnet deploy path depends on full Hardhat compile surface
- Location: `scripts/deploy_mainnet.py:141`
- Evidence:
  - Deployment wrapper runs `npm run compile`, which compiles the broader repository Solidity surface (including test/fuzz contracts).
- Impact:
  - Non-production harness regressions can block production deploy workflow.
- Remediation plan:
  1. Introduce a deploy-specific compile target/config that only builds deployable contracts and required artifacts.
  2. Keep full compile in CI gates, but decouple mainnet deploy execution from test-harness compile failures.
  3. Add a deploy preflight command that validates expected artifact presence deterministically.

### F-03 (Low) — Verifier initialization allows zero-length validator sets (operator footgun)
- Location: `contracts/verifiers/SoraBeefyLightClientVerifier.sol:138`
- Evidence:
  - `initialize()` stores provided sets without non-zero length guard.
  - Later signature verification rejects zero-length sets, resulting in fail-closed liveness loss.
- Impact:
  - Misconfiguration at initialization can permanently block root imports and mint verification until contract replacement.
- Remediation plan:
  1. Add explicit `initialize()` validation for non-zero validator set lengths (and optionally non-zero roots).
  2. Add a test asserting initialization rejects invalid sets up-front.
  3. Update operational runbook with pre-deploy validator-set sanity checks.

### F-04 (Informational) — Digest parser is intentionally strict and may be future-fragile
- Location: `contracts/verifiers/SoraBeefyLightClientVerifier.sol:627`
- Evidence:
  - Parser rejects unknown item kinds/network kinds and requires exact byte consumption.
- Impact:
  - Upstream digest format evolution may cause unexpected fail-closed behavior and liveness incidents.
- Remediation plan:
  1. Document this strictness as an explicit compatibility contract.
  2. Add regression tests for anticipated upstream encoding variants.
  3. If protocol policy allows, consider tolerant parsing that ignores unknown non-SCCP items while retaining strict SCCP commitment checks.

## Remediations Implemented During Audit

### R-01 — Fixed Solidity fuzz harness compile blockers and invariant bug
- Location: `test/fuzz/SccpCodecFuzz.t.sol`
- Changes:
  - Replaced direct calldata-only decode call with `this.decodeExternal(payload)` for memory payload paths.
  - Updated function mutability where required (`pure` -> `view`) for external self-calls.
  - Fixed nonce-sensitivity fuzz test by constructing an explicit second payload struct instead of `memory` alias assignment.
- Result:
  - `npm test` now passes end-to-end.

### R-02 — Removed `eval` from E2E adapter script
- Location: `scripts/sccp_e2e_adapter.sh:26`
- Changes:
  - Converted string command execution to array-based invocation (`"${cmd[@]}"`).
- Result:
  - Reduced shell-injection surface and preserved existing behavior.

## Validation Results
- `npm run compile`: pass
- `npm test`: pass (`85 passing`)
- `npm run test:formal-assisted`: pass
- `npm run test:fuzz:fastcheck`: pass
- `npm run test:fuzz:foundry`: not executable in current environment (`forge` missing)
- `npm run test:fuzz:echidna`: not executable in current environment (`echidna` missing)

## Notes
- Local environment warning observed during Hardhat execution: Node.js `25.6.1` is unsupported by Hardhat recommendations; use Node 22 LTS for CI/local consistency.
