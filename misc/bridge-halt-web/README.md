# SORA bridge emergency halt page

A static page with one button: **HALT ALL BRIDGES**. Guardians press it during an
attack. It submits `bridgeHalt.halt(message)`, which writes the message on chain and
sets the bridge latch that every SORA bridge already obeys. Guardians pay no fee.
For any other account the button has no effect: the chain rejects the transaction
before it reaches a block, so nothing is written and no fee is charged. Only devops
can reopen the bridges, with a Root call.

The page needs runtime 139 or later (the `BridgeHalt` pallet). On older runtimes it
still shows whether the bridges are halted, and says the button is unavailable.

## What is on chain

| Item | Value |
| --- | --- |
| Call | `bridgeHalt.halt(reason: Bytes ≤ 256)`, operational class |
| Latch | raw key `:sora-nexus:recovery:bridge-halt:v1` = `0x3a736f72612d6e657875733a7265636f766572793a6272696467652d68616c743a7631` |
| Record | `bridgeHalt.lastHalt` (guardian, block, message) and the `bridgeHalt.BridgeHalted` event |
| Guardian list | `bridgeHalt.guardians`, changed by the technical committee (more than 2/3) or Root |
| Reopen | `bridgeHalt.resume()`, Root only |

While the latch is present, HASHI (`ethBridge`), `bridgeProxy`, the bridge channels and
apps, the data signer and the multisig verifier refuse new bridge work, and the HASHI
offchain workers stop. Inbound submissions are refused, not consumed, so relayers can
resubmit them after reopening. The latch is the same one the October 2026 recovery set;
any value, even empty, counts as halted.

Admission rules, enforced in transaction validation and again before dispatch:

- signer is a guardian and the bridges are running: accepted, fee 0;
- signer is not a guardian: rejected with `BadSigner` ("Invalid signing address");
- bridges already halted: rejected with `Stale` ("Transaction is outdated");
- any tip: rejected with `Payment`.

A rejected transaction is never included, so it changes no state and costs nothing.
Wrapping `halt` in `utility.batch`, a multisig or the scheduler gets no exemption: the
wrapper pays its usual fee and dispatch still requires a guardian.

### What a halt does not do

- It does not pause the Ethereum, TON or Liberland contracts. Withdrawals the bridge has
  already signed stay redeemable there. Pause those contracts separately.
- External relayers keep running; the chain refuses their submissions.
- It cannot carry a tip. If the page reports the halt as dropped, press again.

## For guardians

1. Use the polkadot{.js}, SubWallet or Talisman browser extension. Do not use the
   Fearless Wallet extension; the page ignores it.
2. Keep about 1 XOR on the guardian account. Halting is free and spends none of it, but
   the account must exist on chain (an empty account is rejected as unable to pay), and
   some wallets show the ordinary fee estimate and refuse to sign if the balance looks
   too low.
3. Before an incident, open the page once and connect the wallet, so the site is already
   authorised. Importing the chain's metadata into the extension (for example through
   polkadot.js apps) lets it show `bridgeHalt.halt` instead of raw bytes when signing.
4. Open the page, check the node and the status, press **Connect wallet**, pick your
   account (guardians are marked ✓), type a short message, press the button, confirm,
   and sign in the wallet. The log shows the block that included the halt, then when it
   became final.

If two guardians press at once, the first halt wins; the second is rejected as already
halted.

## For devops

Manage guardians with a technical-committee motion (more than 2/3) or Root:

```text
bridgeHalt.addGuardian(account)
bridgeHalt.removeGuardian(account)
```

Reopen the bridges with Root only:

```text
bridgeHalt.resume()
```

`system.killStorage(["0x3a736f72612d6e657875733a7265636f766572793a6272696467652d68616c743a7631"])`
has the same effect. Never use `killPrefix` on `:sora-nexus:recovery:`; it deletes
recovery state the chain still reads. Before reopening after an incident, finish the
bridge reconciliation (counterparty nonces, HASHI scan height, peer key rotation).

If the halt was not warranted (for example a guardian key was stolen), remove that
guardian in the same Root call that reopens, or it can halt again at once:
`utility.batchAll([bridgeHalt.removeGuardian(account), bridgeHalt.resume()])`.

Add and fund the guardians before the bridges reopen; the list starts empty. Root on
mainnet goes through a referendum, so check how quickly devops can get a reopening
enacted.

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
