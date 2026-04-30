# Security Audit Report

Date: 2026-04-25

Scope: static review of this Rust/Substrate workspace, runtime pallets, node/RPC wiring, Docker/dev deployment files, and locked Rust dependencies. I did not run a live network, fuzzing campaign, formal verification, or runtime migration simulation.

External references used:

- RustSec describes `cargo audit` as scanning `Cargo.lock` for crates with known security vulnerabilities: https://rustsec.org/
- Polkadot Developer Docs document production RPC hardening such as `--rpc-methods=safe`, connection limits, reverse proxy authentication/rate limiting, and warn that `--unsafe-rpc-external` exposes RPC publicly: https://docs.polkadot.com/node-infrastructure/run-a-node/polkadot-hub-rpc/

## Executive Summary

The most important risks are in consensus/runtime code, not only dependencies. Apollo's per-block hooks do unbounded user iteration and subtract reward balances without checked or saturating arithmetic. That creates credible block-production/chain-availability risk. The runtime also prices extrinsic bytes at zero while allowing 7 MB blocks, which weakens fee-based DoS resistance. The custom bridge multisig pallet contains multiple zero-weight, fee-free calls. The dev Docker stack exposes RPC/database services with unsafe or weak defaults and committed local secrets.

Dependency posture is also weak: `cargo audit` reported 24 vulnerabilities, plus 13 unmaintained advisories, 5 unsound advisories, and 2 yanked crates across 1,405 locked dependencies.

## Findings

### HIGH - Apollo reward counters can underflow in `on_initialize`

Evidence:

- `pallets/apollo-platform/src/lib.rs:1426` calls `update_interests` and `update_rates` on every block.
- `pallets/apollo-platform/src/lib.rs:1430` subtracts `LendingRewardsPerBlock` from `LendingRewards` with plain `-`.
- `pallets/apollo-platform/src/lib.rs:1433` subtracts `BorrowingRewardsPerBlock` from `BorrowingRewards` with plain `-`.

Impact: once a reward bucket falls below its per-block reward, block initialization can panic in overflow-checking builds or wrap in unchecked builds. Either outcome is bad for a consensus-critical hook: panic can halt block execution, while wrap can inflate reward accounting.

Recommendation: use `checked_sub` or `saturating_sub` with explicit error/event handling. Enforce `per_block <= remaining` on reward changes, include tests that advance past reward exhaustion, and consider deriving distribution from escrowed balances instead of mutable counters.

### HIGH - Apollo `on_initialize` performs user-inflatable unbounded iteration

Evidence:

- `pallets/apollo-platform/src/lib.rs:1747` starts `update_interests`.
- `pallets/apollo-platform/src/lib.rs:1759` iterates every `UserLendingInfo` entry for the selected pool.
- `pallets/apollo-platform/src/lib.rs:1776` iterates every `UserBorrowingInfo` entry for the selected pool.
- `pallets/apollo-platform/src/lib.rs:1791` returns the dynamic weight only after doing the work.

Impact: lending/borrowing users can grow the number of entries touched by a consensus-critical block hook. If a pool accumulates enough accounts, block execution can exceed practical time/weight limits and degrade liveness.

Recommendation: replace per-user block hooks with lazy accrual indices, bounded queues, or a fixed `MaxUpdatesPerBlock` scheduler. Charge update work to user extrinsics where possible and benchmark worst-case proof sizes.

### MEDIUM - Extrinsic bytes are free while block length is large

Evidence:

- `common/src/weights.rs:86` allows 7 MB maximum block length.
- `common/src/weights.rs:88` sets `TransactionByteFee` to `0`.
- `runtime/src/lib.rs:1643` sets `LengthToFee = ConstantMultiplier<Balance, ConstU128<0>>`.
- `pallets/xor-fee/src/lib.rs:738-744` also sets `len_fee: 0` for custom-fee calls, so changing `LengthToFee` alone would not cover those paths.

Impact: normal paid extrinsics are still charged by base fee and weight fee. The gap is that encoded byte length is not independently priced, and several custom-fee calls bypass length fees entirely. Large signed extrinsics are bounded by block/pool limits, but not economically priced by length, which increases mempool, networking, and block-construction DoS exposure when a call's benchmarked weight does not fully scale with encoded input size.

Specific proposal:

1. Start with `TransactionByteFee = balance!(0.0000001)` XOR per encoded byte. This adds about `0.0001024` XOR per KiB and about `0.734` XOR for a full 7 MiB block. That is small for normal transactions but no longer lets block bytes be free.
2. Wire the existing parameter into transaction payment:

   ```rust
   type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
   ```

3. In `xor-fee`, add the computed length fee to custom-fee branches instead of hard-coding `len_fee: 0`, unless a specific call is proven to have tightly bounded encoded size.
4. Add tests asserting:
   - `LengthToFee::weight_to_fee(&Weight::from_parts(1024, 0)) == balance!(0.0001024)`.
   - `XorFee::compute_fee(10_000, call, info, 0) > XorFee::compute_fee(100, call, info, 0)` for one standard-fee call.
   - The same monotonic-by-length property for at least one custom-fee call such as `Assets::register`.
5. Re-run fee UX checks and tune the per-byte value only if the resulting fee for common signed extrinsics changes materially.

### MEDIUM - Custom bridge multisig has zero-weight, fee-free operations

Evidence:

- `vendor/pallet-multisig/src/lib.rs:170`, `191`, `263`, `299`, `451`, and `510` mark bridge multisig calls with `Weight::from_parts(0, 0)`.
- `vendor/pallet-multisig/src/lib.rs:247`, `277`, `316`, and `468` return no-fee post-dispatch results for several paths.

Impact: bridge signatories can perform state reads/writes and possibly dispatch calls without paying proportional weight/fees. If a bridge key or participant is compromised, this becomes a cheap state/CPU pressure path.

Recommendation: benchmark and apply real weights, charge fees unless there is a strictly bounded operational reason, cap pending operations, cap signatory counts, and add tests asserting non-zero weight for each public call.

### MEDIUM - Dev deployment exposes unsafe services and committed secrets

Evidence:

- `bridge-docker/docker-compose.sora.yml:12` publishes node ports, and `:23-25` enables `--unsafe-ws-external`, `--unsafe-rpc-external`, and `--rpc-cors all`.
- `bridge-docker/docker-compose.evm.yml:80` contains a hard-coded Ethereum private key.
- `bridge-docker/docker-compose.evm.yml:110-135` exposes Redis and Postgres on host ports; Postgres uses trust auth.
- `bridge-docker/.env:1-3` commits a database URL/password and `SECRET_KEY_BASE`.

Impact: these are acceptable only for isolated local development. If reused for shared staging or production, public RPC, permissive CORS, trust-auth Postgres, and committed keys can lead to account compromise or infrastructure compromise.

Recommendation: bind dev services to `127.0.0.1` by default, require explicit opt-in for public ports, use `--rpc-methods=safe` for exposed nodes, put RPC behind authentication/rate limiting, move secrets to `.env.example`, and add secret scanning in CI.

### MEDIUM-LOW - Liquidation selection uses predictable randomness

Evidence:

- `pallets/kensetsu/src/lib.rs:285-289` documents that CDP selection can be predicted and manipulated in a front-running attack.
- `runtime/src/lib.rs:2379-2382` wires Kensetsu randomness to `RandomnessCollectiveFlip`.

Impact: if liquidation ordering has economic value, block authors or observers may be able to bias/anticipate liquidation selection.

Recommendation: use stronger randomness or a deterministic, manipulation-resistant liquidation ordering. If this is accepted by design, document the economic bound and add tests for the intended behavior.

### LOW - Bridge bootstrap copies a secret into offchain DB

Evidence:

- `node/src/service.rs:420-434` writes `legacy_secret` to offchain storage under `STORAGE_PEER_SECRET_KEY`.

Impact: the secret originates from local node configuration, but the offchain DB may have a different backup, permission, and incident-response lifecycle than the keystore.

Recommendation: migrate the runtime path to keystore-backed access only, document file permissions/backups for offchain DB, and avoid including the DB in support bundles or broad backups.

## Dependency Audit

Command used:

```sh
cargo audit --db /Users/takemiyamakoto/.cargo/advisory-db/advisory-db-3157b0e258782691 --json > /tmp/sora2-cargo-audit.json || true
```

Result summary:

- Advisory database: 1,058 advisories, last updated `2026-04-25T11:01:07-04:00`.
- Lockfile: 1,405 dependencies.
- Vulnerabilities: 24.
- Warnings: 13 unmaintained, 5 unsound, 2 yanked.

Notable vulnerable packages:

- `wasmtime 35.0.0`: multiple 2026 advisories including sandbox escape/data leak/OOB/panic issues. It is pulled through `sc-executor-wasmtime`.
- `rustls-webpki 0.101.7` and `0.103.10`: certificate validation and CRL parsing advisories.
- `ring 0.16.20`: vulnerability plus unmaintained pre-0.17 warning.
- `curve25519-dalek 2.1.3` and `3.2.0`: timing variability advisory.

Notable warnings:

- Unmaintained: `async-std`, `libsecp256k1`, `parity-wasm`, `parity-util-mem`, `paste`, `proc-macro-error`, `ring 0.16.20`, and others.
- Unsound: `atty`, `keccak`, `lru`, `rand 0.8.5`, `rand 0.9.2`.
- Yanked: `core2 0.4.0`, `keccak 0.1.5`.

Recommendation: upgrade the Polkadot SDK and transitive dependency set to versions that pull patched Wasmtime, rustls-webpki, ring, and curve25519-dalek. Add `cargo audit` or `cargo deny` to CI with a documented allowlist for advisories whose exploitability is demonstrably not in scope.

## Follow-Up Priority

1. Fix Apollo underflow and unbounded hook work.
2. Add real bridge multisig weights and review all zero-weight or `pays_no` calls.
3. Introduce non-zero byte fees and validate transaction-pool limits.
4. Upgrade vulnerable dependencies and add advisory scanning to CI.
5. Harden Docker/dev defaults so unsafe exposure requires explicit local-only intent.
