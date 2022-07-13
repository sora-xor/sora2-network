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
