# SORA bridge emergency halt page

A static page with one button: **HALT BRIDGE WORKERS**. Guardians press it during an
attack. It submits `bridgeHalt.halt(message)`, which writes a numbered halt on chain.
The HASHI offchain worker on every bridge node stops as soon as it sees the halt and
stays stopped until devops restart that node with `--bridge-resume-after-halt <number>`.
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

Halts are numbered from 1. Each HASHI bridge node compares the signal with the halt its
operator last confirmed, which the node keeps in its offchain database. While the two
differ, the worker does nothing: it signs no withdrawals and imports no deposits. The
node's log names the halt, and the Prometheus gauge
`eth_bridge_guardian_halt_stopped` holds its number until acknowledged, including while
Root's separate recovery latch is set. The exporter reads the current best-chain signal
and local acknowledgement. A zero value means no guardian halt is outstanding.
The corrected spec-141 candidate requires the node guard that keeps one HASHI pass
active until it finishes, even when an Ethereum RPC stalls beyond the original lease.
Deploy the corrected node binary before enacting this candidate. Peers providing only
the earlier signing guard remain idle on the corrected runtime. The node checks the current chain state when a
worker signs or submits bridge transactions. Approvals already handed to the pool
before a halt can still propagate and be included afterward.

Root's recovery latch, raw key `:sora-nexus:recovery:bridge-halt:v1`, is separate: a halt
neither sets it nor needs it to be clear. While governance keeps it set, the bridge
pallets refuse new bridge work and the HASHI workers stop whatever the halts say. The
October 2026 recovery set it; any value, even empty, counts.

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
  wait until the workers run again. When the calls themselves must stop, governance sets
  Root's latch.
- The relayers of the EVM, TON and Liberland bridges do not read the signal yet. Devops
  stop them on their servers.
- It does not pause the Ethereum, TON or Liberland contracts. Withdrawals the bridge has
  already signed stay redeemable there. Pause those contracts separately.
- It cannot carry a tip. If the page reports the halt as dropped, press again.

## For guardians

1. Use a browser extension wallet: Fearless Wallet (SORA's own, recommended),
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

On each HASHI peer, wait for the halt to finalize and confirm that bridge work can
resume. Start the peer with its usual recovery chain specification and the halt number
shown on the console. For example, to acknowledge halt 3:

```sh
framenode <usual flags> --offchain-worker always --bridge-resume-after-halt 3
```

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

When a trustworthy pre-outage cursor is unavailable, set
`"recovery_scan_start": "genesis"` to scan missing networks from Ethereum block 0.
Use a sidechain RPC serving historical logs for the replay. Once the guardian halt
is finalized, acknowledge it with the restart flags above.

The log line `Acknowledged guardian halt #3` confirms that halt on this peer. Check the peer's
bootstrap, signing-key, and guardian-halt gauges. Root's recovery latch and peer
membership still govern whether the worker can process requests. An old or future
halt number is refused and the node logs the reason. The next halt requires its own
acknowledgement.

If the halt was not warranted (for example a guardian key was stolen), remove that
guardian before restarting the workers, or it can halt again a minute later.

### Root's latch

Remove Root's recovery latch only with Root, and only that key:
`system.killStorage(["0x3a736f72612d6e657875733a7265636f766572793a6272696467652d68616c743a7631"])`.
Never use `killPrefix` on `:sora-nexus:recovery:`; it deletes recovery state the chain
still reads. Before reopening after an incident, finish the bridge reconciliation
(counterparty nonces, HASHI scan height, peer key rotation).

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
(SORA mainnet). Guardians will use this page during an incident, when phishing links
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
