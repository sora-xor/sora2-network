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

The coefficients in `src/weights.rs` are composed charges calibrated on October
31, 2026 using **compiled Wasmtime 36.0.20** with pooling copy-on-write, the current
runtime Wasm, and the operator-selected **Apple M4 Max, 16 CPUs, 128 GiB** reference
host. FRAME's generated regression fits are retained separately from the applied
charges, which include additional margin for all observed timings, including
shared-host outliers. They are not presented as an automatically generated
drop-in implementation of the pallet's multidimensional weight trait.

The [calibration evidence](../../../../misc/bridge-security-review-2026-10-30/followup/beefy-wasm-calibration-2026-10-31/evidence.json)
retains the exact runtime Wasm and chain spec, node/runtime/source hashes,
compiler and host information, command, raw results, generated FRAME fits,
pre-calibration charges and applied calibrated charges. There were 20 samples
for initialization and 20 steps with 20 repetitions for each of seven variable
cases: **2,820 timed samples** in total. The full submission fixture includes
1,024 distinct synthetic public signing addresses, 382 signatures, a 64 KiB
commitment containing 21,829 small payload entries, 63 MMR nodes, a full root
history, and an authority-set rotation. Setup and proof construction are excluded
from the measured dispatch.

| Case | Maximum observed time across its range | Minimum applied-model margin across all rows |
| --- | ---: | ---: |
| Initialization | 0.017 ms | 5.88x |
| Complete submission | 47.613 ms | 7.28x |
| Signature recovery | 15.106 ms | 12.73x |
| Exhausted sampling budget | 23.342 ms | 2.90x |
| Merkle hashing | 78.827 ms | 2.25x |
| Commitment bytes | 1.717 ms | 6.53x |
| Tiny payload containers, two size scans and hash | 5.601 ms | 2.22x |
| Complete digest/MMR verification | 1.090 ms | 2.95x |

The component comparison includes the fixed dispatch allowance, excludes
database charges, and uses a lower bound on the charge for complete submissions.
Some largest wall-clock observations occurred below the largest input; the host
was shared with other workloads. These are observed calibration margins, not
portable timing guarantees. The measured full submission used six reads and
five writes; the model reserves nine reads and six writes.

The applied submission model charges 5 ms fixed work, 8 microseconds per allowed
sampling attempt, 2.6 ms per signature, 14 microseconds per Merkle node, and
120 ns per encoded commitment byte, plus database work. At 1,024 validators the
most expensive admitted dimensions, including claim padding, charge
1,311,074,320,000 ps before database costs and 1,311,899,320,000 ps with the runtime's
database charges. The normal maximum extrinsic is 1,459,891,843,000 ps, already
excluding its base and initialization allowance. The remaining 147,992,523,000 ps
also accommodates the runtime integration test's explicit 100 ms extension
reserve. Oversized dimensions can receive `Weight::MAX` for admission rejection.

Reproduce the Wasm measurements and check every row against the current model
from the repository root, using a new evidence directory:

```sh
scripts/with_llvm_env.sh cargo build --locked --release -p framenode \
  --features wip,runtime-benchmarks
bash vendor/sora2-common/pallets/beefy-light-client/tools/benchmark-wasm.sh \
  /tmp/beefy-wasm-calibration
python3 vendor/sora2-common/pallets/beefy-light-client/tools/validate-wasm-results.py \
  /tmp/beefy-wasm-calibration/raw-results.json \
  /tmp/beefy-wasm-calibration/calibrated-envelope.json
```

The runner uses the existing warm release target and snapshots the runtime
artifact before execution. It fails if the node or recorded source inputs change
during measurement. The verifier requires every recorded timing to fit at least
twice within its composed allowance and checks the measured storage counts.
Rerun calibration if the engine, reference hardware, resource limits or measured
verification algorithm changes.

The earlier native diagnostic remains available:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test \
  --manifest-path vendor/sora2-common/Cargo.toml \
  -p beefy-light-client --lib --locked native_resource_weight_calibration \
  -- --ignored --nocapture
```

`BEEFY_CALIBRATION_VALIDATORS=1024` restricts the diagnostic to the maximum set.
Fixture generation is excluded. Native unoptimized measurements are evidence
about this machine, not a Wasm or reference-hardware guarantee.

On October 30, the native 1,024-validator fixture took at most 258,864,000 ns
across ten measured submissions. Its charge at that time was 1,172,156,400,000 ps.
A proposed 2,048-validator configuration was rejected after a 2.14-second native
observation exceeded its proposed charge; it is not validated support. Those
native measurements preceded the compiled-Wasm calibration above.

`src/benchmarking.rs` provides executable FRAME benchmarks for initialization,
complete submissions, ECDSA recovery, exhausted sampling budgets, Merkle hashing,
commitment bytes and containers, and complete digest/MMR verification. They are registered in runtime benchmark
metadata when both `wip` and `runtime-benchmarks` are enabled. Test their fixtures
locally with:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test \
  --manifest-path vendor/sora2-common/Cargo.toml \
  -p beefy-light-client --lib --locked --features runtime-benchmarks benchmarking
```

The final pallet suite passed 37 tests, including all eight benchmark fixtures,
with the native diagnostic intentionally ignored. Source tests also check sample
compatibility, rejected inputs, monotonic charging and block-budget headroom.
The optional runtime test checks the configured database charges and actual
normal extrinsic limit with its additional extension reserve.
