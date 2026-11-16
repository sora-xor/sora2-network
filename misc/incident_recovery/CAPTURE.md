# SORA Nexus checkpoint capture

`capture_checkpoint.mjs` reads the complete top trie and every discovered default child
trie at block **28,191,817**. It accepts only `wss://mof2.sora.org` or an explicit loopback
WebSocket origin. It has no transaction, signing, administration or remote write methods.

```sh
node --test misc/incident_recovery/test_capture_checkpoint.mjs
node misc/incident_recovery/capture_checkpoint.mjs \
  --endpoint wss://mof2.sora.org \
  --output artifacts/recovery-28191817/checkpoint
```

Use Node 24 or newer with the built-in WebSocket API. The script has no external package
dependencies. Default pages contain at most 512 keys, and value RPC reads are grouped
into 128-key requests with at most four groups in flight. The endpoint must provide
archive state for the pinned checkpoint.
Oversized archive responses are subdivided sequentially; an individual value that
still exceeds the endpoint limit stops capture. Uncompressed pages are bounded at
256 MiB, including historical preimage pages; readers must enforce the same bound.
Fallback child-value reads run at most 32 individual requests concurrently.

## Resume and integrity

Run the same command to resume. `capture-identity.json` pins checkpoint, endpoint and
page size; differing settings are rejected. Completed page files are immutable and
validated before reuse. An interrupted `.partial-*` file is ignored. Missing, duplicate,
unordered or mismatched keys/values stop the capture. The cursor only advances after a
page and its directory have been fsynced. A live-process lock prevents two writers;
a lock belonging to a dead local process can be replaced automatically.

`pages/top/NNNNNNNN.jsonl.gz` contains a JSON header followed by `["0xkey","0xvalue"]`
rows. Child tries use `pages/child-<sha256-of-full-child-key-string>/` with the same format.
Headers bind the checkpoint hash, trie, page number, previous cursor, last key and count.
Each traversal ends with an explicit empty terminal page. Unknown child-trie kinds or
unavailable child pagination prevent a completion claim rather than being skipped.

The capture also records the checkpoint and parent headers, checkpoint block, runtime
version, exact SCALE metadata and deployed runtime bytes. These are public evidence;
no private keys or credentials are read.

## What completion means

`capture-complete.json` records all page hashes and counts only after every trie finishes.
It deliberately leaves `stateRootVerified`, `headerHashVerified`, `consensusProofVerified`
and `historyComplete` false. The independent verifier must recompute trie roots and the
header hash and validate consensus evidence before the snapshot becomes release evidence.
The parent header alone is not a full pre-checkpoint database or complete chain history.
Do not turn these flags on by editing the capture manifest; retain a separate verifier report.
