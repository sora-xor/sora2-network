# Substrate Polkaswap Parachain



DevOps

* [scripts/inside_docker.sh](scripts/inside_docker.sh) file contains information about needed packages
* socat (for unix socket script passing)
* nodejs (needed to run polkadot-js-api)
* yarn (needed to install polkadot-js-api)
* polkadot-js-api
* rustup
* glibc
* zlib (for rustup)
* git
* gnugrep
* gnuset
* gawk
* gnumake
* findutils
* gnutar
* wget
* utils-linux (getopt command)



# Overview

Parachain pallets, node and runtime for substrate Polkaswap
FIXME.



# System requirements (minimal)

* CPU - 1 core

* RAM - 1 GB (with swap enabled)

* Disk - FIXME GB for database on test stand.

* Network - FIXME

# Build, test & run

## Prepare

### In user environment
* Install `rustup` command
* Install `yarn` command

```
yarn global add @polkadot/api-cli
rustup update nightly || exit 1
rustup target add wasm32-unknown-unknown --toolchain nightly || exit 1
rustup update stable || exit 1
```

### In docker
```make docker-build-image```
or<br/>
```docker build -t soraneo-develop-nix .```


## Build

### In user environment
```make cargo-build-release```<br/>
or<br/>
```cargo build --release```

### In docker
```make docker-build-release```<br/>
or<br/>
```./scripts/docker_compose_up.sh --with-last-commit --run "cargo build --release"```



## Test

### In user enviroment
```make cargo-test-release```<br/>
or<br/>
```cargo test --release```

### In docker
```make docker-test-release```<br/>
or<br/>
```./scripts/docker_compose_up.sh --with-last-commit --run "cargo test --release"```



## Run

```
# Manual run of collator
./target/release/parachain-collator \
    --tmp --validator --alice --ws-port 9944 --port 30333 \
    --parachain-id 200 -- --chain ./misc/rococo-custom.json
```

```
# Manual run of parachain fullnode
./target/release/parachain-collator \
    --tmp --alice --ws-port 9944 --port 30333 \
    --parachain-id 200 -- --chain ./misc/rococo-custom.json
```

```
# Automatic run of local testnet by script for a given commit
make a397f7451d80205abf5e535ecee95073ad49e369
```

```
# Debug version of command
make a397f7451d80205abf5e535ecee95073ad49e369-debug
```

```
# Automatic run of local testnet by script for last commit
make docker-localtestnet
```

```
# Debug version of command
make docker-localtestnet-debug
```



# Integration

Search for an architecture diagram here:

FIXME



# Configuration parameters

FIXME



# Endpoints

FIXME





# Logging

You can set logging level using the environment variable

```export RUST_LOG="sc_rpc=trace"```

You can print logs

```tail -f /tmp/rococo-localtestnet-logs-*/parachain_200_fullnode_0.log```


# Docker containers

### run to test
```
docker build -f housekeeping/docker/develop/Dockerfile -t soraneo-develop .
docker run -ti -v $(pwd):/app -w /app --rm soraneo-develop cargo build --release
```

# Usage localtestnet.sh script
```
Usage: ./scripts/localtestnet.sh [OPTIONS]...
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

# Problems with debug build

Now it is recommended to use cargo build --release for parachain binary
because problem exist with extrinsics in debug mode and release mode is needed
of course this problem with debug will be fixed in future


# Monitoring

## Healthcheck

FIXME



# Storage

FIXME



# Scaling

FIXME



# Queue (optional)

FIXME
