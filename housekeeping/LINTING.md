# Rust lint checks

Run `./housekeeping/clippy.sh` to use the same checks as CI. The runner uses
`--locked --all-targets` and treats warnings as errors. It reuses Cargo's existing
target directory and skips the nested Wasm build. All three modes run even when
an earlier mode fails; the runner returns the first failing exit status. It also
works when invoked by its absolute path from outside the checkout.

| Mode | Packages | Additional features |
| --- | --- | --- |
| `mainnet` | `framenode`, `framenode-runtime`, `eth-bridge` | None |
| `try-runtime` | `framenode`, `framenode-runtime`, `eth-bridge` | `try-runtime` |
| `extended` | Entire workspace | `private-net,stage,wip,runtime-benchmarks` |

Production modes select the node, runtime, and Ethereum bridge explicitly.
Checking the entire workspace would enable `private-net` through other members'
dev-dependencies, even without a `--features` flag. The mainnet mode covers real
runtime migrations; the try-runtime mode also covers migration validation hooks.
The extended mode checks every workspace member and preserves development and
benchmark coverage.

Before each production check, the runner inspects the resolved runtime feature
graph with `cargo tree`. It refuses development or benchmark runtime features,
and verifies that the runtime's `try-runtime` feature is enabled only in the
try-runtime mode. This prevents dependency changes from silently reducing
production coverage.

For a focused rerun, use `./housekeeping/clippy.sh mainnet`,
`./housekeeping/clippy.sh try-runtime`, or `./housekeeping/clippy.sh extended`.
Production modes must not enable `runtime-benchmarks`: that feature substitutes
mock runtime migrations. CI records each mode under a separate SARIF category
while retaining the required `static_analysis` job.

Use `--message-format=json` for diagnostic consumers. Progress messages go to
stderr. When piping diagnostics, use `set -o pipefail` so a successful report
converter cannot conceal a Clippy failure. `./housekeeping/test-clippy.sh` checks
the runner's arguments, feature separation, JSON output, and failure propagation
without compiling Rust.

First-party lint policy lives in the root `Cargo.toml` under
`[workspace.lints]`. Adoption starts with the runtime, node, and Ethereum bridge;
other first-party crates can opt in incrementally with `[lints] workspace = true`.
The adopted crates enforce correctness, suspicious-code, and performance checks;
they temporarily defer the style and complexity groups while legacy findings
are addressed incrementally. Exceptions for SDK error types and test event types
are scoped to individual functions and include a reason.
Keep exceptions scoped to the affected lint and code, and document why they are
needed. Avoid new blanket `allow(clippy::all)` attributes. Vendored dependencies
retain their upstream lint policies and do not inherit the first-party policy.
