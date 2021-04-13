# Substrate Polkaswap Parachain

Parachain pallets, node and runtime for substrate Polkaswap.

## Run

```bash
./scripts/localtestnet.sh [OPTIONS]
```

```
Run local test net, downloading and (re)building on demand
  -h, --help                     Show usage message
  -k, --keep-logdir              Do not remove logdir after end of script work
  -r, --relay-nodes [n]          Number of relay nodes to run (default 2)
  -p, --parachain-fullnodes [n]  Number of parachain node to run (default 2)
  -c, --collator-nodes [n]       Number of collator nodes to run (default 4)
  -f, --force-rebuild-parachain   Remove parachain binary and rebuild with fresh commit (as additional test)
  -s, --skip-build               Skip build is parachain binary is exist
  -l, --logdir-pattern [pat]     Pattern of temporary logdir (default "/tmp/rococo-localtestnet-logs-XXXXXXXX")
  -d, --cache-dir [dir]          Cache dir to incremental backups of target dir (default "/tmp/parachain_cargo_target_build_cache")
  -j, --just-compile-deps        Compile dependencies and exit
  -e, --exit-after-success       Exit after success parachain block producing
  -g, --use-parachain-debug-build          Use debug build for parachain binary
  -w, --use-polkadot-debug-build           Use debug build for polkadot binary
```

### Minimal System requirements

* CPU - 1 core
* RAM - 1 GB (with swap enabled)
* Disk - FIXME GB for database on test stand.
* Network - FIXME

### Prerequisites

#### System

Needed for UNIX sockets passing inside build scripts and other system functionality.

```bash
sudo apt install socat glibc zlib git gnugrep gnuset gawk gnumake findutils gnutar wget utils-linux
```

#### Node and Node Package Manager 

Needed to build and run polkadot-js-api.

```bash
sudo snap install node --classic --channel=10
```

#### Rust

Needed to build SoraNeo node.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup update stable
```

## Logging

You can set logging level using the environment variable:

```
export RUST_LOG="sc_rpc=trace"
```

You can print logs:

```
tail -f /tmp/rococo-localtestnet-logs-*/parachain_200_fullnode_0.log
```

## Troubleshooting

1. If you are getting errors after running the nodes, try installing the specified version of the polkadot JS library:
```
npm install -g @polkadot/api-cli@0.22.2-7 --prefix ./tmp/local
```

2. If you have troubles with compilation, try using the oldest nightly version:
```
rustup uninstall nigthly
rustup default nightly-2021-03-11
rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-11
```