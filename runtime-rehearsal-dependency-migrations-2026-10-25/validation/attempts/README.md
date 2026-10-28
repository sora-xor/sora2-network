# Superseded rehearsal evidence

Only the top-level `validation/rehearsal-run.json` and its hash-bound reports are
authoritative for the final candidate run. These directories preserve earlier
outputs; a passed individual stage inside an older failed run is not final
production-semantics evidence.

The first strict compatibility attempt rejected the omitted `__data_end` linker
global. Source review established that both inspected runtime hosts read preserved
`__heap_base`, and the gate now permits only that exact non-callable removal.

The next attempt applied signed fixtures directly in an upgrade block whose
runtime returned `OnlyInherents`. Its early passed equivocation stage is therefore
superseded. The unfunded payout guard caught framework migration-progress events.
The final harness finalizes that timestamp-only upgrade block, records migration
completion, initializes a following `AllExtrinsics` block, and follows the node
block builder's storage-transaction rollback on invalid transactions. Final
independent verification also rejects any raw invalid diff outside three System
accounting fields.

Earlier capture/version/endpoint negative checks and bounded bridge samples are
retained for diagnosis; they do not replace the final complete inventory or
hash-bound runtime gates.

## Retained run attempts

- `prior-to-20261024T173632Z-9936c32b`: run `20261024T173411Z-6b414a90`, `failed`. Stage Exact candidate compatibility failed with exit 1; see /Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-dependency-migrations-2026-10-25/validation/wasm-compatibility.log
- `prior-to-20261024T174836Z-fa72028d`: run `20261024T173632Z-9936c32b`, `failed`. Stage Fee policy and bridge fixtures failed with exit 1; see /Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-dependency-migrations-2026-10-25/validation/fee-policy-wasm-rehearsal.log
