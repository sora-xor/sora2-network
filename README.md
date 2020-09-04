# Substrate Polkaswap Parachain



DevOps

* Npm required

* Nodejs required

* OpenSSL required

FIXME: versions and outer dependencies



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

curl https://sh.rustup.rs -sSf | sh

rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup update stable


## Build

make cargo-build-release



## Test

make cargo-test-release



## Run

make localtestnet-run



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

