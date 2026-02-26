# SCCP Proof Helper Fuzzing

This directory contains `cargo-fuzz` targets for SCCP proof helper modules.

## Targets

- `evm_proof_helpers`: exercises `sccp::evm_proof` RLP/MPT helper functions.
- `tron_proof_helpers`: exercises `sccp::tron_proof` TRON header/signature helpers.

## Run locally

Prerequisite: use a Rust/Cargo toolchain that supports Rust 2024-edition crates in transitive dependencies.

```bash
cargo install cargo-fuzz
cd pallets/sccp/fuzz
cargo fuzz run evm_proof_helpers
cargo fuzz run tron_proof_helpers
```

To run for a bounded duration in CI-like checks:

```bash
cd pallets/sccp/fuzz
cargo fuzz run evm_proof_helpers -- -max_total_time=60
cargo fuzz run tron_proof_helpers -- -max_total_time=60
```
