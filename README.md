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
docker-compose up


## Build

### In user environment
make cargo-build-release

### In docker
make docker-build-release



## Test

### In user enviroment
make cargo-test-release

### In docker
make docker-test-release



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
# Automatic run of local test net by script for last commit
make docker-localtestnet
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


# Monitoring

## Healthcheck

FIXME



# Storage

FIXME



# Scaling

FIXME



# Queue (optional)

FIXME
