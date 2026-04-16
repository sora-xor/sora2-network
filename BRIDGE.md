# Quick start for Trustless Ethereum bridge

## Run ethereum node

[Docs](https://www.ethdocs.org/en/latest/network/test-networks.html#setting-up-a-local-private-testnet)

### Build geth and ethkey from source

```bash
git clone https://github.com/ethereum/go-ethereum
cd go-ethereum
go build cmd/geth
go build cmd/ethkey
```

### Prepare genesis

Example genesis

```json
{
  "config": {
    "chainId": 4224,
    "homesteadBlock": 0,
    "eip150Block": 0,
    "eip150Hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "eip155Block": 0,
    "eip158Block": 0,
    "byzantiumBlock": 0,
    "constantinopleBlock": 0,
    "petersburgBlock": 0,
    "istanbulBlock": 0,
    "muirGlacierBlock": 0,
    "grayGlacierBlock": 0,
    "berlinBlock": 0,
    "londonBlock": 0,
    "ethash": {}
  },
  "nonce": "0x0",
  "timestamp": "0x615d5464",
  "extraData": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "gasLimit": "0xffffffffffff",
  "difficulty": "0x40000",
  "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "coinbase": "0x0000000000000000000000000000000000000000",
  "alloc": {
    "90F8bf6A479f320ead074411a4B0e7944Ea8c9C1": {
      "balance": "0x10000000000000000000000"
    }
  },
  "number": "0x0",
  "gasUsed": "0x0",
  "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "baseFeePerGas": null
}
```

Specify your accounts in `alloc` field

Save genesis to `soranet.json`

### Run node

Init chain

```bash
./geth --datadir data init soranet.json
```

Create account 

```bash
./geth --datadir data account new
```

Run node
```
./geth --networkid 4224 \
     --mine --miner.threads 1 \
     --datadir "./data" \
     --nodiscover --http --http.port "8545" \
     --ws --ws.port "8546" --ws.api "eth,web3,personal,net,debug" \
     --port "30303" --http.corsdomain "*" \
     --nat "any" --http.api eth,web3,personal,net,debug \
     --unlock 0 --password ~/work/soramitsu/ethereum/password.sec \
     --allow-insecure-unlock \
     --verbosity 5 \
     --ipcpath "~/Library/Ethereum/geth.ipc"
```

## Prepare bridge

You should run Ethereum and Sora network

### Deploy Ethereum contracts

Install node modules
```bash
cd ethereum-bridge-contracts
yarn
```

Prepare dotenv
```bash
cp env.template .env
```

Get private key via `ethkey`

```bash
./ethkey inspect --private <ethereum node folder>/data/keystore/<your key data>
```

And set to `GETH_PRIVATE_KEY` var in `.env`

Deploy contracts
```bash
./deploy.sh
```

### Register bridge in Sora

```bash
./bridge-scripts/register-bridge.sh
```

By default it will register local geth ethereum network. If you want to choose different, run the script with `relayer bridge register-bridge`'s parameters (see `--help` for details). For example,

```bash
./bridge-scripts/register-bridge.sh --ropsten
```

If you want to use ERC20 tokens in bridge you should register that tokens. 
Example script placed in `bridge-scripts/register-assets.sh`

## Start relaying messages

### Relay messages from Ethereum
```bash
./bridge-scripts/run-ethereum-relay.sh
```

### Relay messages from Sora
```bash
./bridge-scripts/run-substrate-relay.sh <private-key>
```

## Transfer tokens
Now you can transfer tokens through bridge. 

### Sora to Ethereum
Easiest way is to use `ethApp.burn` and `erc20App.burn` via polkadot.js/apps

### Ethereum to Sora
Example placed in `bridge-scripts/transfer-eth.sh`

## Operational Runbook

### Clearing Stalled Outgoing Requests

If an outgoing request collects signatures but cannot finalize (for example because the bridge account lacks funds), the pallet keeps the existing approvals. Once the underlying issue is fixed, clear the stale signatures before peers vote again:

1. Use a root session (sudo in polkadot.js/apps) and call the extrinsic  
   `ethBridge.resetRequestSignatures(network_id, request_hash)`.
2. Confirm the runtime emits `ethBridge.RequestSignaturesCleared`.
3. The request status should remain `Failed` or `Broken`; resubmit or re-run the approval flow as appropriate.

This explicit reset replaces the old behaviour where signatures were wiped automatically on failure, preventing accidental loss of evidence while still giving operators a deterministic recovery step.

### Diagnosing SORA to Ethereum Approval Stalls

When Ethereum to SORA keeps working but SORA to Ethereum requests remain `Pending` with `0` approvals, focus on the outgoing approval path on the bridge peers.

Check these Prometheus metrics on each peer:

1. `eth_bridge_bootstrap_ready` should be `1`.
2. `eth_bridge_local_signing_key_ready` should be `1`.
3. `eth_bridge_substrate_rpc_configured` should be `1`.
4. `eth_bridge_local_peer_ready{network_id="..."}` should be `1` for the affected network.
5. `eth_bridge_sidechain_rpc_configured{network_id="..."}` can be `0` without blocking outgoing approvals after this fix, but it still indicates a broken Ethereum RPC configuration for incoming and reconciliation flows.
6. `eth_bridge_outgoing_pending_requests{network_id="..."}` shows the queue depth.
7. `eth_bridge_outgoing_zero_approval_requests{network_id="..."}` shows how many outgoing requests are stuck before the first approval.
8. `eth_bridge_outgoing_approval_failure_total{network_id="...",reason="..."}` shows which failure mode is repeating:
   - `no_local_peer_key`
   - `failed_sign`
   - `failed_send_signed_tx`
   - `failed_sidechain_rpc_preflight`

Operational notes:

1. Outgoing requests with zero approvals are retried every 10 finalized blocks.
2. A broken Ethereum RPC endpoint no longer prevents outgoing approvals from being submitted.
3. The bridge channel now clears `QueueTotalGas` after each outbound commit, so EVM outbound batches start with a fresh gas budget.
