# Candidate Wasm validation

The candidate SHA256 is `a39de37798885ee253db508c553aca80321974fdd20fa0830bb94220cb9224c3`.
The captured deployed spec-131 baseline SHA256 is
`db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447`.

`babe-repair-wasm-rehearsal.json` records execution of this exact candidate over
mainnet state pinned to block 27734948,
`0x193b0c9000b1ecf56a883ffc170b6251d9832e478f089ef9052ec1d612a523e4`.
Only `:code` was manually overlaid locally. Actual Wasm runtime calls performed
the upgrade, initialized and finalized three local blocks, and applied their
timestamp inherents. The checks passed:

- The upgrade set the one-time marker and scheduled Plain while preserving the
  active epoch, next epoch, and all three BABE API payloads.
- The first boundary announced Plain once through `NextConfigData`, and the
  second boundary activated it without repeating the announcement.
- Both boundaries promoted the exact previously advertised next epoch. The
  `NextEpochData` authorities and randomness matched the runtime API.
- After the second boundary, `configuration`, `current_epoch`, and `next_epoch`
  all returned Plain with `(1, 4)`. Spec version is 132; transaction version is 131.

This is a runtime execution test. Its synthetic headers are unsealed, its
executor mocks signatures, and it does not test client import or network
consensus. The three consecutive block numbers jump over intervening slots and
election preparation. At the second boundary, the election provider logged
`SnapshotUnavailable` and entered its fallback path. Those messages are retained
in the report and log; this run does not validate a complete validator election.

`wasm-compatibility.json` and `metadata-compatibility.json` record a separate
comparison using subwasm 0.21.3. Existing SCALE layouts were compatible across
81 pallets, 405 calls, 442 events, 967 errors, 485 storage entries, 155 constants,
and 7 signed extensions. No metadata entries were added. Only the encoded
`System.Version` constant changed. Runtime API versions, all 55 host imports,
and 86 exports were unchanged. The import comparison resolves all 54 function
parameter/result signatures from the Wasm type section and checks the imported
memory requirements: 45 minimum pages, no declared maximum, unshared 32-bit
addressing. Parsed names/kinds/order are cross-checked against the independent
WebAssembly engine. Unsupported import/type forms fail the check. The new spec
resets impl version from 2 to 1. These interface checks do not prove host
implementation behavior; execution coverage is limited to the calls listed above.

From the repository root, reproduce with the pinned dependencies:

```sh
npm ci --prefix runtime-upgrade-4.8.10/validation
node runtime-upgrade-4.8.10/validation/rehearse-babe-repair.cjs
node runtime-upgrade-4.8.10/validation/verify-babe-repair-compatibility.cjs
```

The actual recorded run reused the existing exact-version installation:

```sh
SORA_REPAIR_DEPENDENCIES=/tmp/sora-author-replay-20260922/package.json \
  node runtime-upgrade-4.8.10/validation/rehearse-babe-repair.cjs
node runtime-upgrade-4.8.10/validation/verify-babe-repair-compatibility.cjs
```

The replay checks installed dependency versions, records its script and input
hashes, and restricts remote requests to read-only RPC. It uses the public
`https://mof2.sora.org` archive endpoint by default and a disposable public-state
read cache in `/tmp`. Options allow alternate dependency, snapshot, endpoint,
cache, candidate, comparator, and output paths. No private keys, signed
extrinsics, network submissions, or validator hosts were used.
