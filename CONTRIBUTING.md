# Contributing

## Requirements
* Nightly Rust of same version as defined in [housekeeping/docker/develop/Dockerfile](housekeeping/docker/develop/Dockerfile)
 ```
 rustup default set nightly-2021-03-11
 rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-11
 ```

## Steps to do before opening a PR
Unless all the steps are executed, CI will fail the build
* Format the code `cargo fmt`
* Fix all warnings `RUSTFLAGS="-Dwarnings" cargo check`
* Execute tests `RUSTFLAGS="-Dwarnings" cargo test`

## Build

### Docker Image

```bash
make docker-build-image
```

or

```bash
docker build -t soraneo-develop-nix .
```

### Binary release

```bash
make cargo-build-release
```

or

```bash
cargo build --release
```

## Test

### Cargo

```bash
make cargo-test-release
```

or

```bash
cargo test --release
```

### Docker

```bash
make docker-test-release
```

or

```bash
./scripts/docker_compose_up.sh --with-last-commit --run "cargo test --release"
```

## Run

### Docker Compose

```bash
make docker-build-release
```

or

```bash
./scripts/docker_compose_up.sh --with-last-commit --run "cargo build --release"
```

### Manual run of collator

```bash
./target/release/parachain-collator \
    --tmp --validator --alice --ws-port 9944 --port 30333 \
    --parachain-id 200 -- --chain ./misc/rococo-custom.json
```

### Manual run of parachain fullnode

```bash
./target/release/parachain-collator \
    --tmp --alice --ws-port 9944 --port 30333 \
    --parachain-id 200 -- --chain ./misc/rococo-custom.json
```

### Automatic run of local testnet by script for a given commit

```bash
make a397f7451d80205abf5e535ecee95073ad49e369
```

#### Debug version

```bash
make a397f7451d80205abf5e535ecee95073ad49e369-debug
```

### Automatic run of local testnet by script for last commit

```bash
make docker-localtestnet
```

#### Debug version

```bash
make docker-localtestnet-debug
```

### Docker build and run

```bash
docker build -f housekeeping/docker/develop/Dockerfile -t soraneo-develop .
docker run -ti -v $(pwd):/app -w /app --rm soraneo-develop cargo build --release
```
