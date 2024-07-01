<img alt="SORA logo" src="https://static.tildacdn.com/tild3664-3939-4236-b762-306663333564/sora_small.svg"/>

# Overview

This is FRAME-based Substrate node of SORA2.
This repo contains code of node, pallets, runtime.

[CONTRIBUTING.md](CONTRIBUTING.md)

[BRIDGE.md](BRIDGE.md) - Quick start for Ethereum bridge

# System requirements

## Minimum (for example for small docker container)
* CPU 800MHz 1 core.
* RAM 800Mb.
* Disk 2Gb.

## Normal
* CPU 1500MHz 2 cores.
* RAM 4Gb.
* Disk 6Gb.

# System requirement for validator node
* Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz.
* A NVMe solid state drive. Starting around 80GB - 160GB will be okay for the first six months of SORA, but will need to be re-evaluated every six months, as the blockchain grows.
* 32 Gb.

# Quick start

### Dependencies

Follow installation steps for your platform:

https://substrate.dev/docs/en/knowledgebase/getting-started/

### Run tests

```sh
# all
cargo test

# specific pallet
cargo test -p assets
```

### Run local network

```sh
# build binary
cargo build --release --features private-net

# run multiple nodes with local chainspec
./run_script.sh -d -w -r
```
access running network via polkadot.js/apps (select Development -> Local Node, e.g. [link](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2F127.0.0.1%3A9944#/explorer))
#### On macOS
macOS is shipped with the BSD version of `getopt`. If the script behaves incorrectly and does not parse arguments,  
make sure you have installed and set as active the GNU versions of `awk` (or `gawk`) and `getopt`.


### Run benchmarks to generate weights
> For release must be run on hardware matching validator requirements specification.

```sh
# example: run benchmarks for all extrinsics of trading-pair pallet
cargo run --release --bin framenode --features private-net,runtime-benchmarks benchmark pallet --chain=local  --execution=wasm --wasm-execution=compiled --pallet trading_pair --extrinsic "*" --steps 50 --repeat 20 --output ./
```
produces `trading_pair.rs` file in `./` (project root)

### Parse extrinsic data
```
cargo run --bin parse <extrinsic data>

cargo run --bin parse 84aa9d65d15905b5bd071d6a1b6178be15db6bdddc7a91c9b8f52b3049edbeebbd00de47feadb282078c891172cb2035054ba9442c95aa6be1c85124db04b5f751dd41308e3599dcde3523117b08e8defdc776143782a4c436298263945e75bffb0f7500ed12001a0000000000020004000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000025000000000000000000000000000000270100000000000000000000000000000000
```

# Configuration parameters

## Command line

After building with `cargo` binary will be located at `<project_root>/target/debug/framenode` or `<project_root>/target/release/framenode` for --release build.

### Logging

To enable detailed logging node can be run with
`RUST_LOG=debug RUST_BACKTRACE=1` env variables and `-lruntime=debug` flag.

### Exporting chainspec json

Refer to `generate_chain_specs.sh` script.

### Selecting a chain

Use the ```--chain <chainspec>``` option to select the chain. Can be local, dev, staging, test, or a custom chain spec.

### Archive node

An archive node does not prune any block or state data. Use the ```--pruning archive``` flag. Certain types of nodes like validators must run in archive mode. Likewise, all events are cleared from state in each block, so if you want to store events then you will need an archive node.

Note: By default, Validator nodes are in archive mode. If you've already synced the chain not in archive mode, you must first remove the database with polkadot purge-chain and then ensure that you run Polkadot with the ```--pruning=archive``` option.

### Validator node in non-archive mode

Adding the following flags: ```--unsafe-pruning --pruning <NUM OF BLOCKS>```, a reasonable value being 1000. Note that an archive node and non-archive node's databases are not compatible with each other, and to switch you will need to purge the chain data.

### Exporting blocks

To export blocks to a file, use export-blocks. Export in JSON (default) or binary (```--binary true```).

```polkadot export-blocks --from 0 <output_file>```

### RPC ports

Use the ```--rpc-external``` flag to expose RPC ports and ```--ws-external``` to expose websockets. Not all RPC calls are safe to allow and you should use an RPC proxy to filter unsafe calls. Select ports with the ```--rpc-port``` and ```--ws-port``` options. To limit the hosts who can access, use the ```--rpc-cors``` option.

### Offchain worker

Use ```--offchain-worker``` flag to set should execute offchain workers on every block or not
By default it's only enabled for nodes that are authoring new blocks.
[default: WhenValidating]  [possible values: Always, Never, WhenValidating]

### Specify custom base path (storage path)

Flag ```-d```, ```--base-path <PATH>```

### Run a temporary node

Flag ```--tmp```

A temporary directory will be created to store the configuration and will be deleted at the end of the process.

Note: the directory is random per process execution. This directory is used as base path which includes: database, node key and keystore.

### Specify a list of bootnodes

Flag ```--bootnodes <ADDR>...```

## Default ports

* 9933 for HTTP
* 9944 for WS
* 9615 for prometheus
* 30333 p2p traffic

## Other documentation

### Embedded Docs

Once the project has been built, the following command can be used to explore all parameters and
subcommands:

```sh
./target/release/framenode -h
```

### Reading external documentation about ports and flags

* [Alice and Bob Start Blockchain](https://substrate.dev/docs/en/tutorials/start-a-private-network/alicebob)
* [Node Management](https://wiki.polkadot.network/docs/en/build-node-management)

## Useful utilities

- `utils/parse` - parses extrinsic blobs

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

# License

License is original "BSD License" (BSD 4-clause license)
SPDX-License-Identifier: BSD-4-Clause
Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

    Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.
    Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.
    All advertising materials mentioning features or use of this software must display the following acknowledgement: This product includes software developed by Polka Biome Ltd., SORA, and Polkaswap.
    Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
