# Security Best Practices Audit Report — `sccp-tron`

## Executive Summary
This audit found one **critical design-level issue** and multiple **high/medium process issues** that materially affect bridge security posture.

The most severe issue is a direct mismatch with the stated target model (**no governor/operator**): current contracts implement a single privileged control plane that can replace the proof verifier and control incident/censorship paths. In this design, compromise or misuse of the privileged key can lead to unauthorized minting and censorship.

A second major issue is verification coverage: required PR CI does not compile and execute the contract test suite, and the current suite is already failing to compile. This weakens change safety for the mint/proof path.

## Scope & Methodology
- Reviewed: `contracts/`, `scripts/`, `test/`, `.github/workflows/`, build config.
- Executed locally:
  - `npm test` (fails at compile stage)
  - `npm run test:formal-assisted` (passes)
- Attempted analyzer install (Slither/Foundry/Echidna) but blocked by network resolution in this environment.

## Environment Constraints (Coverage Gaps)
- Analyzer/tool install blocked:
  - `pip3 install --user slither-analyzer` failed (host resolution / no index access)
  - Foundry install failed (`curl: Could not resolve host: foundry.paradigm.xyz`)
  - Echidna download failed (network fetch unavailable)
- Local Node version is unsupported by Hardhat warning:
  - `v25.6.1` local vs CI pin at Node `22`.

---

## Critical Findings

### CRIT-001 — Roleless Security Model Is Violated by Privileged Control Plane
**Impact:** A single privileged key/process can unilaterally alter verification trust, censor flows, and (via verifier replacement) enable unauthorized minting.

**Evidence:**
- Router enforces privileged authority and stores governor:
  - [contracts/SccpRouter.sol:96](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:96)
  - [contracts/SccpRouter.sol:101](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:101)
- Security-critical privileged mutators:
  - `setVerifier`: [contracts/SccpRouter.sol:115](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:115)
  - `setInboundDomainPaused`: [contracts/SccpRouter.sol:244](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:244)
  - `setOutboundDomainPaused`: [contracts/SccpRouter.sol:251](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:251)
  - `invalidateInboundMessage`: [contracts/SccpRouter.sol:258](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:258)
- Verifier also has privileged bootstrap/ownership:
  - [contracts/verifiers/SoraBeefyLightClientVerifier.sol:118](/Users/mtakemiya/dev/sccp-tron/contracts/verifiers/SoraBeefyLightClientVerifier.sol:118)
  - [contracts/verifiers/SoraBeefyLightClientVerifier.sol:129](/Users/mtakemiya/dev/sccp-tron/contracts/verifiers/SoraBeefyLightClientVerifier.sol:129)
  - [contracts/verifiers/SoraBeefyLightClientVerifier.sol:138](/Users/mtakemiya/dev/sccp-tron/contracts/verifiers/SoraBeefyLightClientVerifier.sol:138)
- Test demonstrates privileged verifier swap to always-true verifier, then mint succeeds:
  - [contracts/verifiers/AlwaysTrueVerifier.sol:7](/Users/mtakemiya/dev/sccp-tron/contracts/verifiers/AlwaysTrueVerifier.sol:7)
  - [test/sccp-router.test.js:135](/Users/mtakemiya/dev/sccp-tron/test/sccp-router.test.js:135)
  - [test/sccp-router.test.js:138](/Users/mtakemiya/dev/sccp-tron/test/sccp-router.test.js:138)
  - [test/sccp-router.test.js:139](/Users/mtakemiya/dev/sccp-tron/test/sccp-router.test.js:139)

**Why this matters:**
- Under your stated model (“no governor/operator”), this is a direct protocol-level security violation.
- Even under a governed model, verifier replacement is a root-of-trust change and currently has no delay/safeguard.

**Recommended remediation (roleless target):**
1. Remove `governor` authority paths from router and verifier.
2. Make verifier trust root immutable at deployment or selected via non-upgradeable, protocol-validated mechanism.
3. Remove or redesign pause/invalidation controls so they cannot be executed by a centralized actor.
4. If emergency controls must exist, explicitly change the threat model to governed mode and require time-delayed, multi-party controls.

---

## High Findings

### HIGH-001 — Security-Critical CI Path Does Not Compile/Run Contract Tests on PRs
**Impact:** Security regressions in mint/proof logic can be merged without failing required PR checks.

**Evidence:**
- PR workflow runs only `npm run test:formal-assisted`:
  - [\.github/workflows/sccp_formal_assisted.yml:41](/Users/mtakemiya/dev/sccp-tron/.github/workflows/sccp_formal_assisted.yml:41)
  - [\.github/workflows/sccp_formal_assisted.yml:42](/Users/mtakemiya/dev/sccp-tron/.github/workflows/sccp_formal_assisted.yml:42)
- Current `npm test` fails at compile stage:
  - `TypeError: Invalid implicit conversion from bytes memory to bytes calldata`
  - [test/fuzz/SccpCodecFuzz.t.sol:33](/Users/mtakemiya/dev/sccp-tron/test/fuzz/SccpCodecFuzz.t.sol:33)

**Recommended remediation:**
1. Add a required PR job for `npm test` (or at minimum `npm run compile` + core contract tests).
2. Keep nightly fuzz jobs, but do not rely on nightly-only checks for merge protection.
3. Add a short “security smoke” PR job for proof-path tests in `test/sccp-router.test.js` and `test/sora-beefy-light-client-verifier.test.js`.

### HIGH-002 — Root-of-Trust Verifier Can Be Swapped Instantly Without Delay/Two-Step Acceptance
**Impact:** Verification guarantees can change immediately, giving monitors/users no reaction window.

**Evidence:**
- Direct single-call setter with no timelock or pending state:
  - [contracts/SccpRouter.sol:115](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:115)

**Recommended remediation:**
1. Enforce two-step verifier rotation (`proposeVerifier` + `acceptVerifier`) with enforced delay.
2. Emit detailed transition events and block minting during transition if required.
3. If roleless model is strict, remove this capability entirely.

---

## Medium Findings

### MED-001 — GitHub Actions Are Not Fully Commit-Pinned
**Impact:** Tag-based actions can change behavior over time, increasing CI supply-chain risk.

**Evidence:**
- `actions/setup-node@v4` is tag-based, not commit-pinned:
  - [\.github/workflows/sccp_formal_assisted.yml:26](/Users/mtakemiya/dev/sccp-tron/.github/workflows/sccp_formal_assisted.yml:26)
  - [\.github/workflows/sccp_fuzz_nightly.yml:20](/Users/mtakemiya/dev/sccp-tron/.github/workflows/sccp_fuzz_nightly.yml:20)

**Recommended remediation:**
1. Pin all actions to commit SHAs.
2. Add periodic dependency/action bump process with explicit review.

### MED-002 — Toolchain Drift Between Local and CI Reduces Reproducibility
**Impact:** Engineers may see divergent behavior locally versus CI, reducing reliability of security validation.

**Evidence:**
- Hardhat warning indicates local Node `v25.6.1` unsupported for project commands.
- CI uses Node `22` in workflows:
  - [\.github/workflows/sccp_formal_assisted.yml:28](/Users/mtakemiya/dev/sccp-tron/.github/workflows/sccp_formal_assisted.yml:28)
  - [\.github/workflows/sccp_fuzz_nightly.yml:22](/Users/mtakemiya/dev/sccp-tron/.github/workflows/sccp_fuzz_nightly.yml:22)

**Recommended remediation:**
1. Add `engines` in `package.json` (Node 22 LTS range).
2. Add `.nvmrc` or equivalent toolchain lock.
3. Fail early in scripts when unsupported Node versions are used.

---

## Positive Security Notes
- Inbound mint path is fail-closed when verifier is unset:
  - [contracts/SccpRouter.sol:212](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:212)
- Domain allowlist and recipient canonical checks are present in both burn and mint paths:
  - [contracts/SccpRouter.sol:145](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:145)
  - [contracts/SccpRouter.sol:150](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:150)
  - [contracts/SccpRouter.sol:235](/Users/mtakemiya/dev/sccp-tron/contracts/SccpRouter.sol:235)

## Recommended Immediate Priority Order
1. Resolve CRIT-001 by aligning implementation to the intended roleless model.
2. Fix HIGH-001 by making compile/tests required in PR CI and resolving the current compile break.
3. Address HIGH-002 if any governed transition mechanism remains.
4. Apply CI hardening and toolchain locks (MED-001, MED-002).
