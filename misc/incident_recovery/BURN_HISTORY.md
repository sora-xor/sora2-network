# SORA Nexus historical Burn evidence

TS/SS recovery needs the complete event interval **25,043,003 through
28,191,817**. A list from the indexer, an empty RPC response, a call amount or a
declaration of range coverage does not establish that interval's completeness.

`capture_burn_history.mjs` collects sealed headers and extrinsics, and storage
read proofs for `System.Events` and `:code`. The initial parent has a separate
code proof. Every block's event metadata is executed from its **parent's**
authenticated code, including a block that installs an upgrade. Proof nodes are
deduplicated by SHA-256; page files are immutable and resumable.

```sh
node misc/incident_recovery/capture_burn_history.mjs \
  --output /absolute/path/to/new-burn-evidence \
  --first 25043003 --max-pages 1 \
  --verifier /absolute/path/to/incident-recovery \
  --verifier-sha256 REVIEWED_VERIFIER_SHA256
```

Only the approved `mof2.sora.org` endpoint or a loopback archive is permitted.
The explicit page limit bounds each invocation to at most 16 blocks per new
page. With the explicitly SHA-pinned native verifier, capture requests a
combined proof for events and the raw `:` key (`0x3a`) and combines it with the
preceding authenticated code path. The short prefix supplies changing internal
branches above the code leaf without normally retrieving that leaf. Its value
or absence is never used to establish code identity, and different historical
trie shapes can still require the full-proof fallback.
The native helper must prove both `System.Events` and `:code` against the current
header's state root. A missing path triggers one full events/code proof retry;
an invalid proof or a failed full retry aborts. Header hashes and parent links
are checked by the helper, while the final offline verifier independently checks
the complete ancestry and extrinsics through the pinned checkpoint.

The helper retains only nodes visited while proving `:code`, bounded to 4096
nodes and 32 MiB. Unused/event nodes do not accumulate. Reused node bytes were
SHA-256 checked and trie-hashed when first loaded; every later use still requires
the current authenticated root to reach that exact node. Runtime-update digests,
spec versions and unproved RPC code hashes never imply that code is unchanged.
The emitted pages keep the original format and explicitly list every required
node, including reused code nodes. The offline verifier caches those exact
content-addressed code nodes but cannot fill an omitted page reference from a
prior proof. Archives and the shared receipt extractor remain compatible.

Without the verifier options, capture uses the original full-proof path.
Production's state trie version 0 can include the entire runtime code in each
read proof: that mode can transfer terabytes even though disk storage deduplicates
the nodes. Keep each capture invocation bounded and measure actual proof traffic
before planning the complete interval. An approved local archive or retained
database remains preferable for a full capture. This optimization does not
authorize a multi-million-block remote job. The one-block checkpoint sample is
a diagnostic and cannot clear the complete-history release gate.

The native verifier reconstructs storage proof reads, checks extrinsics roots,
links every header to the pinned checkpoint hash/root and executes each unique
runtime's metadata in Wasm. It never selects metadata using spec version alone.
The retained mainnet client substitutes start at parent heights 11,271,782
(spec 57) and 23,234,812 (spec 115). If either spec is encountered at or after
its activation, authenticated stored code is sufficient only when its exact
hash equals the pinned substitute's bytes. Otherwise verification stops and
requires separately authenticated executing-artifact/substitution evidence;
it must not label stored code as executed code. The guard also runs when a
metadata cache entry is reused. No complete local capture currently establishes
whether either substitute is needed in the requested Burn interval.
This historical command stops at the checkpoint, so its last executing parent
is 28,191,816. The recovery spec134 substitute activated at parent 28,191,817
belongs to the separately witnessed live-recovery path, not this historical
extractor.
Missing proof nodes fail. Only a valid absence proof can supply the storage
default empty event vector. Raw event bytes are preserved for the shared
indexer extractor.

```sh
incident-recovery verify-burn-history \
  --manifest /absolute/path/to/burn-evidence/burn-history-manifest.json \
  --output /absolute/path/to/new-verified-burn-evidence
```

The output directory must not exist. A failed run leaves an incomplete directory
without `verification-complete.json`; it must never be treated as verified.
This command accepts a shorter range ending at the checkpoint for diagnostics,
but reports `coverageComplete=false`. It does not verify consensus finality or
authorize a release.

In the companion `polkaswap-indexer` repository,
`src/scripts/extract-burn-history.ts` executes the SHA-pinned verifier afresh and
feeds its authenticated bytes to the same event extractor used by live indexing
and backfill. It rejects incomplete ranges. It then produces the receipt capture
for `prepare-burn-recovery.ts`, which allocates exact TS/SS amounts before
ownership substitution and attacker extinguishment.

Keep commitments acyclic: the recovery policy commits raw event evidence,
mapping, attacker registry and other source inputs. The derived ledger contains
that policy commitment. The outer release binds the ledger, policy, runtime
artifacts and all referenced evidence. **Do not hash the derived ledger back
into the policy it already references.** Allocation needs no post-fork anchor;
the live indexer separately requires a finalized post-fork anchor and executing
metadata witnesses before it admits recovery blocks.

Offline regression tests:

```sh
node --test misc/incident_recovery/test_capture_burn_history.mjs
cargo test -p incident-recovery burn_history
```
