#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

cargo test
cargo test formal_assisted_ -- --nocapture
cargo test --manifest-path verifier-program/Cargo.toml
cargo test --manifest-path program/Cargo.toml --test sccp_flow solana_program_burn_rejects_invalid_inputs_before_account_loading -- --exact
cargo test --manifest-path program/Cargo.toml --test sccp_flow solana_verifier_rejects_duplicate_validator_keys -- --exact
cargo test --manifest-path program/Cargo.toml --test sccp_flow solana_program_mint_from_proof_rejects_local_domain_and_bad_lengths_early -- --exact
