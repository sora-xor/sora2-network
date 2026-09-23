# Validator block production investigation — 22 September 2026

**Follow-up: a BABE configuration defect is now reproduced with the actual client verifier.** The runtime advertises secondary-VRF slots while canonical blocks use secondary-plain. All 13 sampled secondary-plain blocks fail verification under the runtime config and pass when only the slot mode is corrected to Plain; seven primary controls pass. See [ROOT-CAUSE.md](ROOT-CAUSE.md) for the mechanism, historical origin, repair requirements, and per-node confirmation still needed.

The initial runtime-only investigation below confirmed missing authorship for four active validators and ruled out current session disablement at the sampled boundaries. Its successful Wasm replay did not exercise client consensus verification. The live runtime is the exact 4.8.9 release Wasm.

## Live evidence

Read-only snapshot captured at 2026-09-22 10:10:37 UTC from `wss://ws.mof.sora.org`. Historical observations were cross-checked against the same finalized block on `wss://mof2.sora.org`.

- Finalized block: **27,734,948**, hash `0x193b0c9000b1ecf56a883ffc170b6251d9832e478f089ef9052ec1d612a523e4`.
- Genesis: `0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5`.
- Runtime spec version: **131**. Deployed `:code` SHA-256: `db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447`, matching the packaged and tested 4.8.9 Wasm.
- Session **47,369**, active era **7,677**, 15 validators. `Session.disabledValidators=[]`; `Liveness.disabledDuringSession=[47369,[]]`.
- Eleven validators had authored 155 blocks, matching every block from the session start through the snapshot. Four had authored none.
- All 15 current BABE authority keys match the next authority list, queued session keys, and registered next BABE keys at this snapshot. No duplicate keys were found. This checks registration, not the private keys or signing ability on any node.
- `Staking.forceEra=NotForcing`; completed-session evidence includes normal election and era transitions. Disabled sets are empty at all eight observed session ends and their following boundaries.
- Eight boundary event lists contain `ImOnline.SomeOffline`, but no new `Staking.SlashReported` or offences event. The era boundary at block 27,733,301 contains 23 `Staking.Slashed` events, consistent with application of previously deferred penalties. These financial deductions did not leave validators disabled at the observed boundary.

Completed sessions examined: **47,361–47,368**, with the same validator order. Counts below describe this window only.

| Index | Validator stash | Most recent consecutive completed sessions with zero authored blocks |
| --- | --- | --- |
| 2 | `cnSgNFZaSoRGMVqTenCDaPLQhTur42Es5kVL5bQE3jRBHV9un` | 4; authored 7 in the preceding session |
| 6 | `cnULofTPpy7mxwv1jrYtNmVkwjVF6myAmYZmmbAEE3Vy9Sa9y` | All 8 examined |
| 10 | `cnVCuEDifwFEuNxTgf6NxrtwYJ56SEiC4mVse6iLQADpHKxZK` | 1; authored 39 in the preceding session |
| 14 | `cnWjWRxYHVg3wekb4ALe8z3rjiGKMZ5Cc9se5Q5sRkdqBXbee` | All 8 examined |

The public RPC server reports node binary `4.8.6-unknown-aarch64-apple-darwin`. This identifies that RPC server only. It does not establish what binary any affected validator runs.

## Local runtime replay

`replay-authors.cjs` independently replayed one child block from the pinned state for indices **0** (producing control), **2, 6, 10, 14**, using the exact deployed Wasm. All five cases:

1. Completed `Core_initialize_block` and resolved the expected stash.
2. Incremented that stash's `ImOnline.authoredBlocks` counter.
3. Successfully applied an unsigned timestamp inherent.
4. Returned a header from `BlockBuilder_finalize_block`, with an empty disabled set.

This is a runtime execution check, **not a valid BABE block production test**. It uses synthetic `SecondaryPlain` pre-digests while runtime state advertises primary/secondary VRF slots, and the local executor enables mock signature hosts. It does not test VRF eligibility, usable private keys, seals, real proposal contents, networking, or block import. The separate real-header SDK verifier reproduces the configuration failure without mocked cryptography. No blocks or transactions were submitted to the network.

## What remains to inspect

The next diagnostic requires one affected validator's explicit host/RPC endpoint or local operator output. No affected host, local logs, or keystore have been accessed.

The pinned SDK and node service identify these checks:

- The process must run as an authority. BABE startup is gated by `role.is_authority()` in `node/src/service.rs`.
- The node must be sufficiently synced and connected; slot claiming can be skipped during major sync, network-offline conditions, or finality backoff.
- The local keystore must be able to perform BABE VRF signing using the active authority key. The SDK's slot-claiming path treats missing keys and signing errors as unsuccessful claims. `author_hasKey=true` is insufficient: a file can exist while a wrong password or mismatched stored key prevents signing. A false result may also mean a malformed/unreadable key file.
- The node clock, current epoch, best/finalized chain, and proposal/import logs must be compared with the live chain.

Useful existing log messages include `Attempting to claim slot`, `Starting authorship`, `Skipping proposal slot`, `Waiting for the network`, `finality is lagging`, `Too far in the future`, and `slots.unable_authoring_block`. Their presence or absence needs the corresponding logging level; silence at the default level does not prove that a code path was not reached.

## Collect read-only node diagnostics

Copy `node-diagnostics.cjs` and `snapshot.json` together to the affected node, or use an existing local SSH tunnel. With Node.js 18 or newer:

```sh
node node-diagnostics.cjs --stash cnULofTPpy7mxwv1jrYtNmVkwjVF6myAmYZmmbAEE3Vy9Sa9y --rpc http://127.0.0.1:9944 --out node-report.json
```

Replace the stash with the affected validator's address. The collector accepts only a literal loopback HTTP(S) RPC URL, checks genesis before key queries, and uses a fixed read-only RPC allowlist. It reports node version, authority role, health/sync status, best/finalized blocks, reference block agreement, and BABE key presence if the method is exposed. It never exports, inserts, or rotates keys and never submits transactions. It refuses to overwrite an existing output file. The reference keys are tied to the saved snapshot; refresh the public snapshot if the validator set or keys have since changed.

Pair the output with recent BABE/slots logs and an independent system clock synchronization check. Neither a key-presence response nor the successful local runtime replay establishes the root cause on its own.

## Evidence files

- `snapshot.json`: pinned live state; staking ledgers are resolved through `Bonded(stash) -> Ledger(controller)`.
- `completed-sessions.json`: authored counts, keys, disabled sets, and boundary events for eight completed sessions.
- `session-times.json`: chain timestamps delimiting each completed session.
- `author-wasm-replay.json`: all five local runtime replay cases and limitations.
- `*.cjs`: collection/replay scripts. Public-state scripts use temporary dependency installations whose locations are explicit in their source; the node collector itself has no npm dependencies.

The follow-up implementation and release validation are in
[`runtime-upgrade-4.8.10`](../../runtime-upgrade-4.8.10/README.md). It repairs the
proven chain configuration discrepancy and adds an explicit offline cache-repair
command. Individual validator recovery still requires evidence from that node.

### Dependency provenance

The initial public-state collector environment used caret dependency constraints.
Later inspection found SORA type definitions **1.46.3** and Babel runtime **8.0.5**
installed there, despite some capture records listing requested versions 1.27.7
and 7.28.4. Those records are retained as captured. Raw BABE SCALE storage, the
sealed canonical headers, and the exact pinned Rust SDK verifier independently
establish the configuration mismatch. The release rehearsal uses a separate
installation with exact dependencies and asserts the installed versions; its
lockfile and actual version report are retained in the release package.
