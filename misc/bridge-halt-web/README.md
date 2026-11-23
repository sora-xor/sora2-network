# SORA Nexus bridge emergency halt page

A static page with one button: **HALT BRIDGE WORKERS**. Guardians press it during an
attack. It submits `bridgeHalt.halt(message)`, which writes a numbered halt on chain.
The HASHI offchain worker on every bridge node stops as soon as it sees the halt and
stays stopped until DevOps restart that node with its exact local
`--bridge-resume-after-observed-halt <digest>` acknowledgement.
No Root call or vote is needed to bring the bridge back, and guardians cannot restart
anything. Guardians pay no fee. For any other account the button has no effect: the
chain rejects the transaction before it reaches a block, so nothing is written and no
fee is charged.

The page needs runtime 139 or later (the `BridgeHalt` pallet). On older runtimes it
still shows whether Root's recovery latch halts the bridges, and says the button is
unavailable.

## What is on chain

| Item | Value |
| --- | --- |
| Call | `bridgeHalt.halt(reason: Bytes ≤ 256)`, operational class |
| Signal | raw key `:sora-nexus:bridge-halt:latest:v1` = `0x3a736f72612d6e657875733a6272696467652d68616c743a6c61746573743a7631`, holding the newest halt as SCALE `(number: u32, block: u32)` |
| Record | `bridgeHalt.lastHalt` (number, guardian, block, message) and the `bridgeHalt.BridgeHalted` event |
| Cooldown | `bridgeHalt.haltCooldown`: 10 blocks (one minute) from one halt to the next |
| Guardian list | `bridgeHalt.guardians`, changed by the technical committee (more than 2/3) or Root; the upgrade that adds the pallet installs `cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY` and `cnU8UgdM4fs9iiCxkRDW8vp2bx4e68LbePkcuQPv1LERKH5oH` |

Halts are numbered from 1. The node persists each changed observation in its local
offchain database with a genesis-bound, monotonically increasing epoch. A new halt,
rewind, disappearance or unreadable signal leaves the worker stopped until DevOps
acknowledges that exact local epoch. A chain reorg and an ordinary daemon restart
cannot release the stop. The import observer must also catch up to the current best
block before any HASHI key use or local submission is allowed.

The existing `eth_bridge_guardian_halt_stopped` gauge reports the current chain signal
and its runtime acknowledgement. A zero gauge is not proof that the durable local
stop is released: after a reorg removes the chain signal, the local stop remains.
Use the node's observed-halt log and exact acknowledgement result as well.
The corrected spec142 candidate requires the node guard that keeps one HASHI pass
active until it finishes, even when an Ethereum RPC stalls beyond the original lease.
Deploy the corrected node binary before enacting this candidate. Peers providing only
the earlier signing guard remain idle on the corrected runtime. The node checks the current chain state when a
worker signs or submits bridge transactions. Approvals already handed to the pool
before a halt can still propagate and be included afterward.

The October 2026 recovery left the raw key `:sora-nexus:recovery:bridge-halt:v1`.
The corrected OCW-only-halt runtime uses validated recovery completion instead of
that historical key's presence for ordinary bridge admission. Unfinished or malformed
recovery still blocks processing. After completion, each node additionally needs its
own explicit local recovery acknowledgement before its HASHI worker signs. No peer
quorum, governance vote or on-chain resume transaction is needed for that restart.
Installing the corrected runtime on the chain is a separate upgrade prerequisite.

Admission rules, enforced in transaction validation and again before dispatch:

- signer is a guardian and no halt in the last 10 blocks: accepted, fee 0;
- signer is not a guardian: rejected with `BadSigner` ("Invalid signing address");
- a halt in the last 10 blocks: rejected with `Stale` ("Transaction is outdated");
- any tip: rejected with `Payment`.

A rejected transaction is never included, so it changes no state and costs nothing.
Wrapping `halt` in `utility.batch`, a multisig or the scheduler gets no exemption: the
wrapper pays its usual fee and dispatch still requires a guardian.

### What a halt does not do

- It does not make the chain refuse bridge calls. Transfers users submit during a halt
  wait until workers run again. Existing approvals can still complete a quorum, and
  already signed withdrawals remain usable on the external contract.
- The relayers of the EVM, TON and Liberland bridges do not read the signal yet. Devops
  stop them on their servers.
- It does not pause the Ethereum, TON or Liberland contracts. Withdrawals the bridge has
  already signed stay redeemable there. Pause those contracts separately.
- It cannot carry a tip. If the page reports the halt as dropped, press again.

## For guardians

1. Use a browser extension wallet: Fearless Wallet (the SORA Nexus wallet, recommended),
   polkadot{.js}, SubWallet or Talisman. The page lists them the way Polkaswap does, with
   an install link for any that is missing; another injected Substrate wallet is listed
   too.
2. Keep about 1 XOR on the guardian account. Halting is free and spends none of it, but
   the account must exist on chain (an empty account is rejected as unable to pay), and
   some wallets show the ordinary fee estimate and refuse to sign if the balance looks
   too low. Being a guardian needs no earlier transaction; only the balance matters.
3. Before an incident, open the page once and connect the account. The site is then
   authorised in the wallet, and the page remembers the wallet and account in this
   browser and reconnects them on the next visit. Importing the chain's metadata into the
   extension (for example through polkadot.js apps) lets it show `bridgeHalt.halt`
   instead of raw bytes when signing.
4. Open the page, check the node and the status, press **Connect account**, choose the
   wallet and then the account (guardians are listed first and marked **Guardian**),
   type a short message, press the button, confirm, and sign in the wallet. The log
   shows the block that included the halt, then when it became final.

If two guardians press within a minute, the first halt stands and the second is rejected
as outdated. A press after that records the next halt number and stops every worker
again, including any that devops restarted in between.

The status shows a halt as current for 24 hours. The chain does not record when devops
restart the workers, so after that the page shows only when the last halt was.

## For devops

Manage guardians with a technical-committee motion (more than 2/3) or Root:

```text
bridgeHalt.addGuardian(account)
bridgeHalt.removeGuardian(account)
```

### Restarting the workers after a halt

On each HASHI peer, review the incident and pending work. Wait for its current best
and finalized guardian states to agree, then restart with the exact observed-halt
digest printed by that node:

```sh
framenode <usual flags> --offchain-worker always --bridge-resume-after-observed-halt 0x<64-hex-digit-context-hash>
```

This works even when a reorg removed the observed signal, after the new absence is
finalized. The digest covers the node's genesis, local epoch and observed guardian
state. A later change requires a different digest, so retaining an old flag in a
service file cannot release it. The legacy `--bridge-resume-after-halt <number>` is
accepted only as an idempotent compatibility check after an exact release; it cannot
release a pending stop. On the first upgrade, an existing guardian record requires
one digest acknowledgement even if an older node saved its numeric acknowledgement.
No signing key is used and no on-chain transaction is submitted by this procedure.

A replacement peer needs its current HASHI key in the node keystore, a local HTTP RPC
listener, and `bridge/eth.json` in the recovery chain configuration directory. Each
configured network needs its first unscanned Ethereum height. The node preserves
existing SCALE `u64` cursors and validates the complete set before publishing the
peer marker. Missing or malformed cursors cannot default to the current Ethereum tip.

For an empty or partial database, use a trusted, reconciled pre-outage snapshot of a
peer that continuously scanned these networks. Serve the **snapshot copy** on loopback
with workers stopped and its local offchain read RPC available. Export the cursor
manifest for the required network IDs (here, network 0):

```sh
python3 misc/incident_recovery/export_hashi_scan_cursors.py \
  --rpc http://127.0.0.1:9944 --output eth-scan-cursors.json 0
```

The exporter refuses a moving snapshot or outstanding bridge retries. Only use heights
known to precede all unreconciled deposits. A valid chain anchor verifies the source
fork; the operator's trusted snapshot supplies the actual scan progress. Copy the
file into the replacement chain's `bridge/` directory. In `bridge/eth.json`, set
`"recovery_scan_start": {"backup": {"sha256": "0x<exported SHA-256>"}}` alongside
`networks`. First sync the replacement through the source block without bridge
bootstrap. On restart with `--offchain-worker always`, the node verifies the exact
file digest, genesis hash, and source block against its own finalized history before
initializing any missing network. Existing heights are never advanced by the backup.

When a trustworthy pre-outage cursor is unavailable, an explicitly audited
reconstruction can be used by a node build containing the reconstruction support
described below. Otherwise, set
`"recovery_scan_start": "genesis"` to scan missing networks from Ethereum block 0.
Use a sidechain RPC serving historical logs for the replay. Once the guardian halt
is finalized, acknowledge it with the restart flags above.

#### Audited reconstruction of a missing cursor

This host-node extension adds an explicit `reconstructed` mode for SORA Nexus.
The originally released 4.8.14 binary does **not** support it. Deploy a separately
reviewed build containing this extension before using this configuration. It
changes neither the runtime nor the worker, signing or halt rules.

A reconstruction is a new operator decision based on reconciled evidence. It is
**not** a recovered pre-outage peer cursor and must never be labelled as a
`hashi-scan-cursors-v1` backup. Before choosing a height, review the complete
inventory of relevant deposits strictly below it against replay state at a
canonical finalized native block. The chosen height must be inclusive of the
earliest unreconciled deposit. Include provider/proof limitations, unresolved
identities, retained retry state and any work outside this scanner's scope in
the audit. A cursor audit does not authorize payments, establish entitlement,
clear attacker-linked requests, or release a halt.

Store the reviewed JSON audit manifest unchanged as
`bridge/eth-scan-reconstruction-audit.json`. It should bind the evidence files,
native genesis/header/state root/runtime/metadata, Ethereum chain and inventory
ranges, exact contracts and Deposit decoder, scanner source/build, per-network
heights, reconciliation method, origin and time of the new derivation, and its
limitations. The node checks the manifest's digest and that it is a nonempty
JSON object. **It does not execute the audit or independently prove its
completeness or conclusions.** The operator must review the manifest and all
referenced evidence before pinning the document below.

Store a separate `bridge/eth-scan-reconstruction.json` with this strict schema
(replace all angle-bracket placeholders; heights below are illustrative):

```json
{
  "format": "hashi-scan-reconstruction-v1",
  "genesisHash": "0x<32-byte native genesis hash>",
  "auditAnchorNumber": 123,
  "auditAnchorHash": "0x<32-byte finalized native audit block hash>",
  "runtimeCodeHash": "0x<Blake2-256 of native :code at the audit anchor>",
  "scanner": "eth-bridge-main-contract-deposits-v1",
  "depositTopic": "0x<Deposit(bytes32,uint256,address,bytes32) event topic>",
  "configuredNetworkContracts": {"0": "0x<20-byte BridgeContractAddress for network 0>"},
  "nextHeights": {"0": 100},
  "auditManifestSha256": "0x<SHA-256 of exact audit manifest bytes>"
}
```

Pin the SHA-256 of the exact reconstruction document bytes in `bridge/eth.json`
alongside `networks`:

```json
"recovery_scan_start": {"reconstructed": {"sha256": "0x<document SHA-256>"}}
```

The network sets in both maps must exactly equal the configured networks, even
when some cursors already exist. This scanner consumes only `Deposit` logs from
each network's current `BridgeContractAddress`. Legacy XOR/VAL or retired
contracts are not implicitly covered; inventory evidence spanning them is only
a superset of this scanner's coverage. Any scanner semantics change requires a
new scope identifier and a new audit.

Before initializing a missing cursor, the node verifies both document hashes,
schema and scanner scope, genesis and canonical finalized audit anchor, and the
runtime code and nonzero contracts at both that anchor and the current best
block. Missing/pruned state or a mismatch refuses startup. Sync through the
audit anchor first with workers disabled; retain its state for this check.
Every check completes before any cursor/configuration activation is written.
Existing valid cursors are authoritative and never rewound or advanced; malformed
ones refuse bootstrap. If every configured cursor already exists, no evidence
file is loaded. These are the same existing-cursor rules as `backup` mode.

This procedure only prepares local scan state. Keep the workers stopped during the
test, and do not add a recovery or guardian acknowledgement as part of
installing the reconstruction. Any eventual release of the bridge requires its
separate transaction review and operator decision.

The log line `SORA Nexus observed halt epoch ... acknowledged locally` confirms the
local release. Bootstrap, signing-key readiness, recovery completion, the separate
recovery acknowledgement and peer membership still govern whether a worker can
process requests. Malformed local state or an unverified database write keeps signing
and submission closed. Do not delete the observed-halt record to bypass a stop.
Only observations successfully persisted and read back can survive a process crash.
An observation persistence error keeps the current process closed; this cannot
preserve an unwritten observation across a crash or repair a failed storage medium.
Keep the node stopped until its storage and retained state have been checked.

If the halt was not warranted (for example a guardian key was stolen), remove that
guardian before restarting the workers, or it can halt again a minute later.

### Prioritizing confirmed Ethereum deposits on spec 142

On a SORA v2 peer with an initialized chronological cursor, put
`priority-scan.json` beside `bridge/eth.json`. Set `format` to
`eth-deposit-priority-blocks-v1`, `genesisHash` to the SORA v2 genesis hash,
`networkId` to `0`, `ethereumChainId` to `1`, and `bridgeContract` to the
network's main Hashi contract. The `blocks` array accepts 1–256 distinct positive
Ethereum block numbers in increasing order. Select blocks whose confirmed
main-contract Deposit logs and native disposition you have verified.

Pass the SHA-256 of the exact JSON file bytes as `--bridge-priority-scan 0x<sha256>`
on the peer's normal reviewed `--offchain-worker always` restart. Selected passes
scan one block through the spec 142 worker's usual receipt, replay and peer
multisig checks. Chronological passes continue from the peer's existing cursor.
The completion log records a local scan. Check the finalized mapping and incoming
transfer status to establish settlement.

While the plan is armed, every Hashi signature and local pool submission,
including later retries, requires the pinned runtime and contract. Retire a
completed or stale plan by removing the option on a normal reviewed restart.

To also follow new deposits, set the format to `eth-deposit-priority-follow-v1`
and add `reconciledThrough`, the last Ethereum block in the reconciled range.
List any unsettled deposit blocks from that range in `blocks`, or use an empty
array if every deposit has settled. Pin the exact file
digest on each peer. The secondary cursor starts at the next block and persists
under this manifest's digest. Changing the file starts a new secondary cursor.

Every other worker pass is reserved for ordinary history. The remaining passes
share the listed blocks and the forward scan. Ordinary forward passes cover up
to 16 blocks. After a provider limit expires, spec 142 may probe up to 30 blocks
once before resuming its ordinary range. The node saves the progress reported by
the runtime after a clean worker release. Unavailable or contradictory receipt evidence pauses
advancement. Spec 142 waits for 30 Ethereum confirmations and checks the receipt
against the observed log. An Ethereum reorganization deeper than that window
requires reconciliation against the canonical chain. Track each deposit's
finalized SORA v2 import to establish delivery.

### Local restart after completed recovery

The corrected node logs the completed recovery context digest while its HASHI worker
is stopped. Review the transactions, retry state and scanner position, then restart
only the intended peer with that exact digest:

```sh
framenode <usual flags> --offchain-worker always --bridge-resume-after-recovery 0x<64-hex-digit-context-hash>
```

The node accepts the digest only when its best and finalized recovery contexts agree.
It stores a versioned acknowledgement in its local offchain database, bound to the
chain genesis, policies, recovery anchor, terminal audit and completion counters.
An ordinary restart preserves that acknowledgement without creating or refreshing it.
A changed recovery context needs a new explicit digest. The setting does not enable
workers by itself, change chain storage, install keys or acknowledge a guardian halt.

When there is also an outstanding guardian halt, use its separate
`--bridge-resume-after-observed-halt <digest>` acknowledgement after finality. The next guardian
halt stops signing and submission again. Leave Germany and Canada configured with
`--offchain-worker never`; acknowledging France must not alter their configuration.
Keep the seven-request node quarantine in place. One France signature can complete
previously collected approvals, so one enabled worker is not a no-settlement guarantee.

The old France test watcher requires the historical latch to remain set, the guardian
record to remain absent, and activity/cursor metrics to stay fixed. It must be replaced
as part of operational activation; otherwise it deliberately disables OCW when work
begins. Preserve its fail-closed response to changed binary, configuration or node
identity, without treating legitimate cursor or transaction progress as a failure.

No recovery storage keys are cleared by this procedure. Never use `killPrefix` on
`:sora-nexus:recovery:`; those records still establish completion and reconciliation.

The upgrade that adds BridgeHalt installs two guardians,
`cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY` and
`cnU8UgdM4fs9iiCxkRDW8vp2bx4e68LbePkcuQPv1LERKH5oH`: the runtime lists them in
`BridgeHaltInitialGuardians`, and `migrations::SeedBridgeHaltGuardians` adds them once, so
a later upgrade does not re-add one that governance removes. Add any other guardians with
the motion above, and fund every guardian before the bridges reopen.

## Hosting

Serve the folder from any static HTTPS host. Browser extensions do not inject into
`file://` pages, so do not open `index.html` directly. The page loads nothing from
other origins; its Content-Security-Policy allows scripts only from its own origin and
WebSocket connections only to the two configured nodes and local nodes. When you change
the node list in `config.js`, change `connect-src` in `index.html` to match.

Have the host send `Content-Security-Policy: frame-ancestors 'none'` (or
`X-Frame-Options: DENY`); a meta tag cannot set it. The page also refuses to run inside a
frame.

`config.js` holds the node list and the genesis hash of the chain the page may sign on
(SORA Nexus mainnet). Guardians will use this page during an incident, when phishing links
are likely, so the page:

- ignores `?rpc=` unless it points at a node on the same computer;
- turns the button off when the node serves a different genesis hash;
- refuses to sign unless the call bytes are exactly `BridgeHalt.halt` (pallet index
  122, call index 0) with the typed message, so a node serving forged metadata cannot
  make a guardian sign some other call.

If the pallet index ever changes, update `HALT_CALL_PREFIX` in `app.js`.

Local test against a development chain:

```sh
cd misc/bridge-halt-web && python3 -m http.server 8090
# open http://127.0.0.1:8090/?dev&rpc=ws://127.0.0.1:9944
```

With `?dev` and a local endpoint only, the page offers the well-known development keys
(`//Alice`, `//Bob`, `//Charlie`, `//Dave`) so it can be tested without an extension.

## Vendored libraries

`vendor/` holds the official browser bundles of `@polkadot/api` and `@polkadot/types`
16.5.4 and `@polkadot/util`, `@polkadot/util-crypto` and `@polkadot/keyring` 14.0.3, the
versions Polkaswap uses with this chain. `fetch-vendor.sh` downloads them again from the
npm registry, checks each tarball against the registry's SHA-512 integrity and each
bundle against `vendor/SHA256SUMS`. `shim.js` supplies the two members the API bundle
expects from `@polkadot/types` modules that have no browser bundle (a constant and the
`unwrapStorageSi` helper, copied from 16.5.4); re-check it when upgrading.

`identicon.js` is the Polkadot identicon from `@polkadot/ui-shared` 3.16.7
(`icons/polkadot.js`, Apache-2.0), ported unchanged so an account shows the same circles
as in the wallet. `img/` holds the wallet logos Polkaswap uses
(`src/lib/soraneo-wallet/src/assets/img`).
