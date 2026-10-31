# SORA Nexus Ethereum deposit source inventory

`capture_hashi_deposits.cjs` collects a resumable, read-only inventory from the
explicitly selected public endpoint from a fixed allowlist: PublicNode at
`https://ethereum-rpc.publicnode.com` [dRPC](https://drpc.org/docs/ethereum-api)
at `https://eth.drpc.org/`, or [Tenderly](https://docs.tenderly.co/node-rpc/overview#public-endpoint-limits)
at `https://mainnet.gateway.tenderly.co`. It preserves the
existing source-provider and HASHI peer-authentication boundaries. Its output
does not authorize incoming credits or change bridge infrastructure.

The preparation step binds the authenticated checkpoint registry, its native
capture-verification receipt, and the existing raw public Ethereum identity
responses. It re-encodes the retained finalized header using the fixed existing
EVM kernel and checks its hash. Ethereum chain ID 1 and the
[mainnet genesis hash in go-ethereum](https://raw.githubusercontent.com/ethereum/go-ethereum/master/params/config.go)
are fixed. The three checkpoint addresses are fixed separately from HASHI's
internal network ID 0. The input graph is rederived and rehashed on every run;
there are no caller-supplied verification switches.

A dRPC preparation requires `--provider drpc --provider-probe-directory PATH`.
It binds the separate raw probe records for chain ID, genesis, the same frozen
header and the exact three-address log query at blocks 0–99. That limited probe
does not establish complete archive capability. Code bytes remain pinned by the
original frozen checkpoint evidence and are checked again against the selected
provider before any range query. Plans record the baseline identity observations
separately from the selected transport provider.

A Tenderly preparation uses `--provider tenderly --provider-probe-directory PATH`
and binds that provider's separate chain/genesis/frozen-header and blocks 0–49,999
probe. Its documented 3,000-log query cap is treated as a truncation boundary:
responses with 3,000 or more logs require a smaller range, never acceptance.
The six `x-tdly-*` bandwidth/rate/reset headers and `retry-after` are retained.
Every successful response must include valid quota headers. The first identity
request establishes the current live quota; subsequent requests stop unless the
last reported budget can cover the response bound and a final canonical-head
check. One rate-limit request is reserved for that final check. The collector
does not automatically wait, retry a 429, or claim control of other clients sharing
the public provider's budget. An access/rate error or invalid quota observation
also prevents the final-head request; that run retains no successful end-head
check. A planned budget stop may use its reserved final check. A quota stop preserves evidence and leaves the
inventory incomplete; an independently prepared later run must respect the
provider's reset and remaining allowance.

There is no automatic fallback. A provider change requires a new independently
pinned plan and a separate capture directory; pages and failed calls from the
previous provider remain unchanged. Version-2 plans cannot be silently substituted
into an older capture. Authentication requirements, rate limits and access-denied
responses are preserved as failures, never retried through another provider by
this collector. The first PublicNode attempt was denied archive access with
HTTP 403; selecting the separately probed public dRPC service does not alter that
retained result or claim that either provider has complete historical coverage.

Every run checks chain ID, genesis, the canonical hash at the frozen height and
the three contracts' code at that exact hash. A request is reserved to recheck
that canonical hash by height at the end of each batch, including completion.
Observed drift is retained and blocks further resumption of that capture;
scanned pages are not certified by a failed batch. Finality remains the public
provider's observed `finalized` result. No beacon light client or alternative
peer quorum is introduced, and no cryptographic consensus-finality claim is made.

The requested range is **block 0 through the frozen head**. This deliberately
covers each configured address before and after deployment, without assuming
code presence is monotonic across destruction or redeployment. Queries include
all logs from those three addresses. Deposit locators retain the original
transaction hash as their native message identity; multiple deposits within one
transaction are not deduplicated or converted into invented native IDs. Other
logs remain in the raw evidence for contract-migration and proxy-upgrade review.

The collector does not derive a beneficiary, credit amount, sender or completion
status. It does not parse receipts. The existing fixed receipt kernel and native
source-association tools remain responsible for their respective checks. In
particular, the kernel's present extraction scope accepts bridge and VAL-master
roles; capturing XOR-master logs does not silently widen that acceptance.

Historical bridge addresses and native acceptance epochs still need the native
configuration/request history. Neither absence of a migration log nor scanning
the current addresses establishes that all historical source contracts were
discovered. The capture keeps this condition explicit and false. Unknown or
ambiguous transitions cannot become approved credits through this collector.

Each query and raw response is stored with exclusive creation and hash references.
HTTP errors, RPC errors, null results, malformed logs and incomplete responses
are retained as failures. They never create empty successful ranges. Range-size
errors reduce the next query span; rate limits and missing history stop that
attempt. A single-block failure remains a failure. The default request timeout
is 15 minutes, with no overall timeout.

Committed range pages form a contiguous hash chain. Resume checks their original
queries, raw response hashes and log summaries. Atomic file publication prevents
an interrupted write from appearing as a committed page; unfinished temporary
files stay on disk. A dead controller's lock is preserved as incident evidence
before another controller resumes. A live controller cannot be bypassed.

The root controller should prepare a new, pinned first-batch execution directory
and start with eight requests: six source-identity/code checks, one log query and
the final canonical-head check. A bounded run can finish successfully with `budgetReached: true` while
the complete range remains unfinished. Subsequent runs use the same immutable
plan and capture directory, with separate execution receipts. Do not label that
partial result a complete incoming inventory.

Pure/offline checks:

```sh
node --test misc/incident_recovery/test_evm_kernel.cjs misc/incident_recovery/test_capture_hashi_deposits.cjs
```

The tracked `evm/kernel.cjs` verification body is byte-identical to the reviewed
candidate. Only its dependency bootstrap changed: it resolves the repository's
existing lock-pinned `@noble/hashes` 1.8.0 installation and verifies the same five
loaded files. Source-provenance pins and primary Ethereum trie/RLP golden tests
record that boundary. No absolute installation path or generated rehearsal output
is a source dependency. A clean checkout uses its normal lockfile installation.

The optional retained public-evidence tests are explicitly skipped when their
local inputs are unavailable; the kernel golden tests remain independent of
those artifacts. No network fallback or dependency installation occurs in tests.
