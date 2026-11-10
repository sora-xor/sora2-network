# SORA Nexus bridge payload recovery

The public archive at `wss://mof2.sora.org` supplied the pinned state and historical
blocks. No credentials, signing, submissions, operator endpoints, or Ethereum
RPC endpoints were used.

All 2,061 pending account/hash identities have exact BLAKE2-256 preimages:

- 2,048 are reconstructed from retained load/incoming request pairs. Their only
  difference from the currently encoded import call is the historical pallet
  index byte, 30 instead of 31.
- 13 come from the original `BridgeMultisig.asMulti` extrinsics. A bounded
  archival search located an adjacent absent-to-present storage transition for
  each key. This does not claim the first-ever creation of an operation.
- All payloads fit within 278 bytes. Both runtimes reject 2,049 historical
  payloads; the remaining 12 decode identically. Decoding does not prove that
  dispatch would succeed or that operators are ready.

`bridge-payload-evidence.json` is the complete preimage inventory.
`bridge-request-recovery-source.json` contains the exact retained request bytes.
`bridge-archived-payload-source.json` contains the original storage observations,
blocks, metadata, and extrinsics. `bridge-request-recovery-capture.json` records
the complete 230,144-request scan and pending inventory. The compressed raw page
stream is `bridge-request-recovery-pages.jsonl.gz`; its uncompressed SHA-256 and
size are recorded in the capture report.

The offline verifier independently checks account/hash coverage, exact source
arguments, the one-byte historical transformation, preimage hashes and lengths,
historical extrinsic roundtrips, header hashes, ordered extrinsics trie roots,
adjacent storage observations, and deployed/candidate decoding. It trusts the
approved endpoint's archived metadata and storage responses; the capture does
not include cryptographic state-trie proofs for those responses.

From this directory, using the validated Node 24 runtime:

```sh
node --test test-bridge-payload-recovery.cjs
node verify-bridge-payload-recovery.cjs bridge-payload-evidence.json bridge-request-recovery-source.json bridge-payload-recovery-verification.json bridge-archived-payload-source.json bridge-request-recovery-capture.json
```

The two `collect-bridge-payload-*.cjs` files retain the bounded public collection
procedures. They write intermediate files beside themselves; copy them into a
separate evidence directory before a fresh collection. The request collector
uses a 300,000-request cap and streamed pages. The archive collector consumes its
`full-recovery.json`, performs at most 25 binary-search rounds with concurrency
four, and checks complete historical extrinsic encoding. Legacy metadata needs
the explicit `Timepoint = BridgeTimepoint` alias.

Historical preimages must never be rewritten in place or treated as newly
authorized transactions. Runtime activation and any quorum cleanup remain
separate actions.
