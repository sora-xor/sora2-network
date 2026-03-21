# Commit Split Plan

This branch contains broad hardening work. The following split keeps commit intent clear.

## Commit 1: Fuzz + Runtime Reliability

Message:
`fuzz: stabilize echidna/foundry pipeline and hardhat node runtime selection`

Suggested paths:
- `package.json`
- `echidna.yaml`
- `foundry.toml`
- `scripts/fuzz_echidna.sh`
- `scripts/fuzz_foundry.sh`
- `scripts/run_hardhat.sh`
- `scripts/select_node22_path.sh`
- `scripts/compile_deploy_contracts.mjs`
- `test/fuzz/**`
- `contracts/echidna/**`
- `scripts/test_fuzz_nightly.sh`
- `.nvmrc`

## Commit 2: CI Orchestration + Repo Hygiene

Message:
`ci: add formal/fuzz wrappers and enforce repository hygiene checks`

Suggested paths:
- `scripts/check_repo_hygiene.sh`
- `scripts/test_ci_formal.sh`
- `scripts/test_ci_fuzz.sh`
- `scripts/test_ci_all.sh`
- `scripts/test_formal_assisted.sh`
- `scripts/test_deploy_scripts.sh`
- `.gitignore`
- `.github/workflows/sccp_formal_assisted.yml`

## Commit 3: Supply-Chain Hardening

Message:
`ci(fuzz): verify echidna artifact checksum before install`

Suggested paths:
- `.github/workflows/sccp_fuzz_nightly.yml`

## Optional Docs Commit

Message:
`docs: add hardening release notes`

Suggested paths:
- `README.md`
- `SPEC.md`
- `RELEASE_NOTES.md`
