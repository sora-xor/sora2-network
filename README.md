# Substrate Polkaswap Parachain



DevOps

* [scripts/inside_docker.sh](scripts/inside_docker.sh) file contain information about needed packages
* socat
* nodejs
* yarn
* rustup
* glibc
* zlib
* git
* gnugrep
* gnuset
* gawk
* gnumake
* findutils
* gnutar
* wget



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

* Install `rustup` command
* Install `yarn` command

```
yarn global add @polkadot/api-cli
rustup update nightly || exit 1
rustup target add wasm32-unknown-unknown --toolchain nightly || exit 1
rustup update stable || exit 1
```


## Build

make cargo-build-release



## Test

make cargo-test-release



## Run

```
# Manual run of collator
./target/releaseparachain-collator \
    --tmp --validator --alice --ws-port 9944 --port 30333 \
    --parachain-id 200 -- --chain ./misc/rococo-custom.json
```

```
# Manual run of parachain fullnode
./target/releaseparachain-collator \
    --tmp --alice --ws-port 9944 --port 30333 \
    --parachain-id 200 -- --chain ./misc/rococo-custom.json
```

```
# Automatic run of local test net by script
./scripts/localtestnet.sh
```



# Integration

Search for a architecture diagram here:

FIXME



# Configuration parameters

FIXME



# Endpoints

FIXME





# Logging

You can set loggin level using environment variable

```export RUST_LOG="sc_rpc=trace"```

You can see log files

```tail -f /tmp/rococo-localtestnet-logs-*/parachain_200_fullnode_0.log```



# Monitoring

## Healthcheck

FIXME



# Storage

FIXME



# Scaling

FIXME



# Queue (optional)

FIXME

