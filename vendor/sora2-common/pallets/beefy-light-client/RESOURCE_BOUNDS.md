# SORA Nexus experimental BEEFY resource accounting

The pallet remains behind the runtime's `wip` feature. Its resource checks and
weights are source hardening, not approval to enable an unaudited bridge network.

`Config::MaxValidators` is an explicit **remote** validator-set limit. The SORA
Nexus runtime currently configures 1,024, independently of its local staking and
authority limits. Before enabling the bridge, verify the remote network's current
and next validator sets fit this value and the runtime's normal extrinsic budget.
Changing this limit also requires recalibrating the worst-case weights.

No remote set is initialized by the checked-in chain specs. The retained public
October 25 metadata does not include `BeefyLightClient`; this optional limit does
not change the active legacy bridge's validator acceptance. That historical
snapshot does not establish which future remote network an operator will enable.

For a selected remote network, the read-only checker pins its finalized block,
checks both BEEFY authority vectors, and retains the header, storage values and
read proof. It also reports the remote configured authority maximum separately:
current sets fitting 1,024 does not guarantee future sets will fit. Supply an
explicit remote endpoint and a Node project with `@polkadot/api` installed:

```sh
node vendor/sora2-common/pallets/beefy-light-client/tools/check-remote-validators.cjs \
  --endpoint "$REMOTE_BEEFY_RPC" \
  --api-root runtime-rehearsal-sdk2609-2026-10-25/validation \
  --output remote-validator-check.json --max-validators 1024
```

This tool does not select a network, send transactions, or change bridge state.
Exit code 2 means the pinned current or next set is empty or exceeds the limit;
exit code 1 means the check could not be completed. It refuses to overwrite an
existing evidence file. No live remote-network compatibility result is claimed
until an endpoint and network are selected and this check is run.

The following bounds preserve the existing SCALE call/proof encoding:

| Work dimension | Bound |
| --- | --- |
| Current and next authority sets | `1..=MaxValidators` |
| Claim bitfield | At most `MaxValidators + 7` bits; padding must be zero |
| Signatures and related outer vectors | At most `ceil(MaxValidators * 22 / 59)` |
| Signature | Exactly 65 bytes |
| Validator Merkle path | At most 32 siblings (u32 leaf index) |
| MMR path | Fewer than 64 siblings (u64 order) |
| Encoded commitment | At most 64 KiB |
| Message digest | At most 1,024 logs |
| Sampler hash attempts | At most `16 * validator_count + 128` |

Sampling retains the legacy rejection sequence, including wrapping `seed + i`
arithmetic used by release Wasm. A budget exhaustion returns an explicit error;
it never silently chooses a different set of signers. Existing accepted samples
therefore require the same signatures. The runtime API returns its existing empty
result on error, without allocating a second attacker-sized buffer.

The weight calculator rejects oversized outer vectors before iterating them.
Accepted submissions charge signature recovery, the entire sampling budget,
Merkle nodes, commitment bytes, and database operations. Inbound verification
charges MMR nodes, digest logs, and database reads. Both bridge channel paths add
the verifier's weight through the existing `MultiVerifier` forwarding code.

The coefficients in `src/weights.rs` are **provisional engineering charges**, not
fabricated generated-benchmark output. At 1,024 validators the most expensive
admitted dimensions charge about 1.193 seconds before database costs, below the
test's 1.4-second ceiling within the runtime's normal budget. Invalid proof
dimensions may be assigned `Weight::MAX` so transaction admission rejects them.

Reproduce the native diagnostic measurements from the repository root:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test \
  --manifest-path vendor/sora2-common/Cargo.toml \
  -p beefy-light-client --lib --locked native_resource_weight_calibration \
  -- --ignored --nocapture
```

`BEEFY_CALIBRATION_VALIDATORS=1024` restricts the diagnostic to the maximum set.
Fixture generation is excluded. Native unoptimized measurements are evidence
about this machine, not a Wasm or reference-hardware guarantee.

On October 30, 2026, the final 1,024-validator fixture on an Apple M4 Max took at
most 258,864,000 ns across ten measured submissions. Its declared charge was
1,172,156,400,000 ps (about 4.5 times that observation). A proposed 2,048-validator
configuration was rejected after a 2.14-second native observation exceeded its
proposed charge; it is not presented as validated support. The machine was shared
with other workloads, and these wall-clock observations are not portable bounds.

`src/benchmarking.rs` provides executable FRAME benchmarks for initialization,
ECDSA recovery, exhausted sampling budgets, Merkle hashing, commitment hashing,
and complete digest/MMR verification. They are registered in runtime benchmark
metadata when both `wip` and `runtime-benchmarks` are enabled. Test their fixtures
locally with:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test \
  --manifest-path vendor/sora2-common/Cargo.toml \
  -p beefy-light-client --lib --locked --features runtime-benchmarks benchmarking
```

Before production activation, run those benchmarks on the intended Wasm engine
and reference hardware, record the measurements, and update the composed
coefficients and supported maximum together. The existing source tests check
sample compatibility, rejected inputs, monotonic charging, and inclusion-budget
headroom; they do not replace that release qualification.
