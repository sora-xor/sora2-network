# Bridge RPC reliability work

Status: source work complete and ready for the existing runtime-upgrade release
process. All 288 bridge tests and 150 native runtime tests pass; four existing
runtime tests are ignored. The no-std Wasm source check passes. No remaining
introduced blocker was found in independent review.

## Scope and goals

The user's September 12, 2026 correction governs this handoff: finish the source
work and report readiness for the existing runtime-upgrade release process.
Release builds, Docker images, SSH, infrastructure changes, deployment, governance
submission, and production operations are outside this task. The earlier goal's
production-deployment wording is superseded. The unavailable external watchdog
source and live incident diagnosis are separate unresolved incident work, not
blockers for this source handoff.

1. Correct reproduced bridge source defects while preserving deposit cursors,
   retry behavior, outgoing approvals, and progress across networks.
2. Review the final changes for runtime compatibility and regressions.
3. Validate the bridge and runtime source, and provide the patch and evidence for
   the established release process.

## Changes

- Failed-import recovery checks the request's own network. An unrelated Ethereum
  request with the same hash cannot remove another network's queued import.
- Recovery runs once per newly observed 200-block period, including skipped
  boundaries. A persistent local high-water mark prevents repeated or regressed
  finalized heads from duplicating recovery attempts.
- HTTP 413 responses to `eth_getLogs` reduce the scan range without advancing the
  cursor. The reduction uses the actual confirmed block span, which may be shorter
  than the configured range. Other methods retain their existing HTTP handling.
- The scanner remembers each network's learned range ceiling and probes a larger
  range after 30 minutes. This prevents successful small scans from immediately
  growing back into repeated provider rejection. A failed larger probe, including
  transport, HTTP 401/429/500, or range failure, restores smaller scans until the
  next probe. A successful larger response removes the learned ceiling. A scan
  shortened by the confirmed tip cannot prematurely clear or defer that ceiling.
- Generic JSON-RPC `-32005` errors no longer trigger range reduction. Explicit
  log-count, result-size, or block-range wording is required, keeping rate and
  account-quota errors on the existing retry path.
- Existing warning prefixes and severity are preserved, with network, method,
  and block-range context added. Provider URLs are removed from request trace
  logging; provider error messages retain debug escaping. This is not a claim
  that every possible sensitive value is redacted from all logs.

Provider documentation supports the distinctions: [QuickNode documents HTTP 413
for excessive block ranges](https://support.quicknode.com/articles/6801664182-understanding-the-413-request-entity-too-large-error),
while [NodeReal documents `-32005` for rate limits, account quotas, and excessive
log counts](https://docs.nodereal.io/docs/support). Neither identifies the
production provider or proves which error caused the reported incident.

## Runtime compatibility

This task changes offchain execution and tests. It adds no on-chain storage
migration, pallet/call/error index, extrinsic encoding, or runtime API change.
The existing cursor and range storage encodings remain unchanged. New local
storage uses a versioned per-network `(u64, u64)` range-limit key and a separate
versioned recovery-generation key; missing state initializes lazily.

The current node uses `WasmExecutor`, and the runtime's `OffchainWorkerApi` calls
`Executive::offchain_worker`. These bridge changes belong to the upgraded runtime
Wasm. The existing candidate source is spec/transaction version 131; this task
adds no further version bump. Independent review found no remaining introduced
runtime compatibility or liveness issue after the failed-probe and logging fixes.

Substantial unrelated runtime and bridge changes predate this task. They are
preserved. The full current runtime suite exercises their integration, including
the registered bridge migration; this is not a claim of exhaustive review of
all unrelated changes.

## Validation

Final results are recorded in [test-results.txt](test-results.txt) and
[validation.json](validation.json). The checks use the existing warm Cargo target
directory and locked dependencies. Native runtime tests disable artifact building;
the Wasm check compiles no-std source without generating a release Wasm package.

```sh
scripts/with_llvm_env.sh cargo test -p eth-bridge --lib --locked
SKIP_WASM_BUILD=1 scripts/with_llvm_env.sh cargo test -p framenode-runtime --lib --locked
SKIP_WASM_BUILD=1 \
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/clang \
AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/llvm-ar \
scripts/with_llvm_env.sh cargo check --locked -p framenode-runtime \
  --no-default-features --target wasm32-unknown-unknown
```

Before-fix runs demonstrate failures for wrong-network recovery, skipped/repeated
recovery boundaries, HTTP 413 classification, repeated range rejection, actual
confirmed-span reduction, failed-probe cooldown, and quota classification.
The regression coverage also checks unchanged 401/429 and non-log 413 behavior,
cursor preservation, outgoing approvals, and progress on another network.

HTTP status tests exercise the actual SDK response-code path through wrapped
externalities. Transport tests invalidate a pending request. Wall-clock timeouts,
streamed-body failures, and live provider behavior are not simulated. Passing
source checks do not prove that no other bugs exist.

The [task-only patch](diagnostics-and-regressions.patch) applies on top of the
task's original dirty working tree, not clean Git HEAD. The manifest records
baseline and tested source hashes, patch and log hashes, and check results.

## Release handoff

The source changes are for the existing runtime-upgrade release process. The
existing `runtime-upgrade-4.8.9` package predates the final bridge changes: its
recorded source hashes do not match either current offchain source file. It must
not be treated as the artifact validated by this handoff. The release process
must build and qualify the final source through its normal gates.

The existing package's local Chopsticks rehearsal stopped because the pinned
remote state had been pruned (`UnknownBlock: State already discarded`). It is not
a passing migration rehearsal or evidence of a runtime failure. It also disabled
offchain workers. This task does not replace that release qualification and does
not generate, modify, or submit the package.

## Separate incident evidence

The **SORA2 Production Watchdog** chat showed alerts and recoveries alternating
while citing the same September 10 log timestamp. The 15-minute error count was
zero while the 60-minute count declined. Recovery messages only asserted container
running/healthy. This establishes repeated notifications for an aging error
window; the precise watchdog defect remains unverified without its source.

`Failed to load handle logs: HttpFetchingError` identifies the deposit log scan,
but groups transport/deadline, non-200 HTTP, response-body, and UTF-8 failures.
It does not identify a provider, prove a deposit backlog, or establish that any
of the reproduced defects caused this particular incident. The HTTP deadline
remains 10 seconds. The [source-location audit](source-location-audit.md) preserves
the bounded watchdog search and outstanding incident evidence separately.

Earlier discovery and failed SSH attempts were outside the corrected scope.
No remote commands ran and no production changes were made.
