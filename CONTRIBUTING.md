# Contributing

## Requirements
* Nightly Rust of same version as defined in [housekeeping/docker/develop/Dockerfile](housekeeping/docker/develop/Dockerfile)
 ```
 rustup default set nightly
 rustup target add wasm32-unknown-unknown --toolchain nightly
 ```

## Steps to take before opening a PR
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

### Local integration network

```bash
./integration-tests/run-local-network.py --build-binary --peers 4
```

The harness starts at least four local validators, waits for peers, block production, and finality, then runs optional commands passed with `--test-command`.
It runs all built-in suites by default; use `--suite smoke` or `--suite none` to narrow the run.
Use `--mock-state adversarial-rewards` with `--suite rewards` to run against live-shaped mocked reward-claim edge cases, including an intentionally inconsistent claimable-greater-than-total holder.
Use `--mock-state adversarial-bridge` with `--suite bridge` to run against mocked bridge asset edge cases.
Use `--mock-state adversarial-market` with `--suite assets` to run against mocked asset, DEX, and trading-pair edge cases.
Use `--mock-state adversarial-vesting` with `--suite rewards` to run against mocked vested reward/crowdloan edge cases.
Use `--mock-state adversarial-iroha` with `--suite iroha-migration` to run against mocked Iroha migration edge cases.
Use `--mock-state adversarial-oracle` with `--suite oracle` to run against mocked Band/oracle rate edge cases; `adversarial-all` includes all mock profiles.

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
