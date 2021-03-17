# Overview.

This is FRAME-based Substrate node of Sora Polkaswap.
Code of node, pallets, runtime.

# System requirements.

## Minimum (for example for small docker container).
* Cpu 800HZ 1 core.
* Ram 800Mb.
* Disk 2Gb.

## Normal.
* Cpu 1500GZ 2 core.
* Ram 4Gb.
* Disk 6Gb.

# Build test run.

## Using nix package manager.

### Install nix package manager.

```sh
curl -L https://nixos.org/nix/install | sh
```
Make sure to follow the instructions output by the script.
The installation script requires that you have sudo access to root.

### Build node binary using nix.

```sh
nix-shell --run "cargo build --release"
```

### Test node using nix.

```sh
nix-shell --run "cargo test --release"
```

### Run node using nix.

```sh
nix-shell --run "cargo run --release -- --dev --tmp"
```

### Running using script, after building.

```sh
./run_script.sh -d -w -r
```

### Single Node Development Chain

Purge any existing dev chain state:

```sh
./target/release/framenode purge-chain --dev
```

Start a dev chain:

```sh
./target/release/framenode --dev
```

Or, start a dev chain with detailed logging:

```sh
RUST_LOG=debug RUST_BACKTRACE=1 ./target/release/framenode -lruntime=debug --dev
```

### Multi-Node Local Testnet

If you want to see the multi-node consensus algorithm in action, refer to
[our Start a Private Network tutorial](https://substrate.dev/docs/en/tutorials/start-a-private-network/).

## Using docker.

### Build image with node binary, cargo test included.

```docker build -t sora/polkaswap/nix .```

### Run this image.

```docker-compose up```

## Using manual rust setup.

### Rust Setup

First, complete the [basic Rust setup instructions](https://github.com/substrate-developer-hub/substrate-node-template/blob/master/doc/rust-setup.md).

### Build

The cargo run command will perform an initial build. Use the following command to build the node without launching it:

```sh
cargo build --release
```

### Run

Use Rust's native cargo command to build and launch the template node:

```sh
cargo run --release -- --dev --tmp
```

# Configuration parameters.

## Command line.

### Selecting a chain.

Use the ```--chain <chainspec>``` option to select the chain. Can be local, dev, staging, test, or a custom chain spec.

### Archive node.

An archive node does not prune any block or state data. Use the ```--pruning archive``` flag. Certain types of nodes like validators must run in archive mode. Likewise, all events are cleared from state in each block, so if you want to store events then you will need an archive node.

Note: By default, Validator nodes are in archive mode. If you've already synced the chain not in archive mode, you must first remove the database with polkadot purge-chain and then ensure that you run Polkadot with the ```--pruning=archive``` option.

### Validator node in non-archive mode.

Adding the following flags: ```--unsafe-pruning --pruning <NUM OF BLOCKS>```, a reasonable value being 1000. Note that an archive node and non-archive node's databases are not compatible with each other, and to switch you will need to purge the chain data.

### Exporting blocks.

To export blocks to a file, use export-blocks. Export in JSON (default) or binary (```--binary true```).

```polkadot export-blocks --from 0 <output_file>```

### RPC ports.

Use the ```--rpc-external``` flag to expose RPC ports and ```--ws-external``` to expose websockets. Not all RPC calls are safe to allow and you should use an RPC proxy to filter unsafe calls. Select ports with the ```--rpc-port``` and ```--ws-port``` options. To limit the hosts who can access, use the ```--rpc-cors``` option.

### Offchain worker.

Use ```--offchain-worker``` flag to set should execute offchain workers on every block or not
By default it's only enabled for nodes that are authoring new blocks.
[default: WhenValidating]  [possible values: Always, Never, WhenValidating]

### Specify custom base path (storage path)

Flag ```-d```, ```--base-path <PATH>```

### Run a temporary node.

Flag ```--tmp```
            
A temporary directory will be created to store the configuration and will be deleted at the end of the process.
            
Note: the directory is random per process execution. This directory is used as base path which includes: database, node key and keystore.

### Specify a list of bootnodes

Flag ```--bootnodes <ADDR>...```

## Default ports.

* 9933 for HTTP
* 9944 for WS
* 9615 for prometheus
* 30333 p2p traffic

## Other documentation.

### Embedded Docs.

Once the project has been built, the following command can be used to explore all parameters and
subcommands:

```sh
./target/release/framenode -h
```

### Reading external documentation about ports and flags.

* [Alice and Bob Start Blockchain](https://substrate.dev/docs/en/tutorials/start-a-private-network/alicebob)
* [Node Management](https://wiki.polkadot.network/docs/en/build-node-management)

## Logging

### The Polkadot client has a number of log targets. The most interesting to users may be:

* afg (Al's Finality Gadget - GRANDPA consensus)
* babe
* telemetry
* txpool
* usage
* Other targets include: db, gossip, peerset, state-db, state-trace, sub-libp2p, trie, wasm-executor, wasm-heap.

### The log levels, from least to most verbose, are:

* error
* warn
* info
* debug
* trace

All targets are set to info logging by default. You can adjust individual log levels using the ```--log``` (```-l``` short) option, for example -l afg=trace,sync=debug or globally with -ldebug.

## Monitoring and Telemetry

### Node status

You can check the node's health via RPC with:

```
curl -H "Content-Type: application/json" --data '{ "jsonrpc":"2.0", "method":"system_health", "params":[],"id":1 }' localhost:9933 
```

### Telemetry & Metrics

The Parity Polkadot client connects to telemetry by default. You can disable it with ```--no-telemetry```, or connect only to specified telemetry servers with the ```--telemetry-url``` option (see the help options for instructions). Connecting to public telemetry may expose information that puts your node at higher risk of attack. You can run your own, private telemetry server or deploy a substrate-telemetry instance to a Kubernetes cluster using this Helm chart.

The node also exposes a Prometheus endpoint by default (disable with ```--no-prometheus```). Substrate has a vizualizing node metrics tutorial which uses this endpoint.


