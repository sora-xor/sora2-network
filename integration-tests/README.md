# Local Integration Tests

`run-local-network.py` starts a disposable SORA network with real local peers and runs built-in integration suites before optional test commands.

By default it:

- materializes the built-in `integration` chain spec;
- starts four validators: Alice, Bob, Charlie, and Dave;
- assigns free local p2p, RPC, and Prometheus ports;
- connects all validators through Alice as a bootnode;
- waits for RPC readiness, peer connections, block production, and GRANDPA finality;
- runs smoke, consensus, bridge, rewards, assets, oracle, Iroha migration, negative RPC, adversarial RPC, adversarial network, and negative launch suites;
- tears nodes down automatically and keeps logs for failed runs.

Passing `--peers 5` or `--peers 6` uses the matching `integration-5` or `integration-6` chain spec so every started peer is in the genesis validator set.

Use `--mock-state adversarial-rewards`, `--mock-state adversarial-bridge`, `--mock-state adversarial-market`, `--mock-state adversarial-vesting`, `--mock-state adversarial-iroha`, `--mock-state adversarial-oracle`, or `--mock-state adversarial-all` to start a matching mock chain spec with extra deterministic edge cases seeded into genesis. Reward fixtures use live-sampled addresses and balances from the SORA network, plus an intentionally inconsistent claimable-greater-than-total holder. Bridge fixtures add a second legacy EVM network with duplicate sidechain token mappings, zero-address token mapping, and unusual precision values. Market fixtures add custom assets, a mock DEX, and adversarial trading pairs. Vested reward fixtures add a fully vested crowdloan with duplicate reward entries and already-rewarded balances so claimable RPCs exercise normalization paths. Iroha fixtures add single-key, zero-balance, referral, multisig, duplicate-key, and unusual-address migration states. Oracle fixtures seed Band symbols and rates at genesis, including stale current-time quotes and a future-dated rate that must return a runtime error.

## Build

The harness needs a `framenode` binary built with `private-net`, because the local and integration chain specs are private-net only.

```bash
./integration-tests/run-local-network.py --build-binary
```

For a release binary:

```bash
./integration-tests/run-local-network.py --build-binary --release
```

To use an existing binary:

```bash
SORA_INTEGRATION_BINARY=target/release/framenode ./integration-tests/run-local-network.py
```

## Run More Tests

The default built-in suite selection is `all`. Run a narrower set with `--suite`, or skip built-in suites with `--suite none`.

```bash
./integration-tests/run-local-network.py --suite smoke --suite negative-rpc
```

Run against mocked adversarial reward state:

```bash
./integration-tests/run-local-network.py --mock-state adversarial-rewards --suite rewards
```

Run against mocked adversarial bridge state:

```bash
./integration-tests/run-local-network.py --mock-state adversarial-bridge --suite bridge
```

Run against mocked adversarial asset and market state:

```bash
./integration-tests/run-local-network.py --mock-state adversarial-market --suite assets
```

Run against mocked adversarial vested reward state:

```bash
./integration-tests/run-local-network.py --mock-state adversarial-vesting --suite rewards
```

Run against mocked adversarial Iroha migration state:

```bash
./integration-tests/run-local-network.py --mock-state adversarial-iroha --suite iroha-migration
```

Run against mocked adversarial oracle state:

```bash
./integration-tests/run-local-network.py --mock-state adversarial-oracle --suite oracle
```

Built-in suites:

- `smoke`: chain identity, runtime version, genesis hash, peer health, validator roles, common block hashes.
- `consensus`: block advancement, finality advancement, block skew, crash-signature log checks.
- `bridge`: bridge RPC exposure, registered bridge assets, proxy apps/assets, historical bridge reads, empty queues, malformed bridge params, adversarial bridge batches.
- `rewards`: rewards/vested rewards RPC exposure, claimable reads, mocked legacy and vested reward states, historical reward reads, malformed claim params, unsigned claim extrinsic rejection, adversarial reward batches.
- `assets`: asset, DEX, and trading-pair RPC exposure, deterministic asset/market reads, mocked market states, historical reads, malformed asset/market params.
- `oracle`: oracle and farming RPC exposure, reward-doubling asset reads, mocked Band symbol/rate states, stale and future-dated quote handling, malformed oracle/farming params, adversarial batches.
- `iroha-migration`: Iroha migration RPC exposure, deterministic needs-migration reads, mocked migration states, historical reads, malformed params, unsigned migration extrinsic rejection, adversarial batches.
- `negative-rpc`: unknown methods, invalid params, malformed extrinsics, malformed JSON, mixed batches, HTTP method misuse.
- `adversarial-rpc`: invalid batch barrages, repeated bad extrinsics, large invalid extrinsic, invalid reserved-peer mutations, post-barrage health.
- `adversarial-network`: non-authority validator joining and wrong-chain peer isolation.
- `negative-launch`: missing chain spec, unknown chain alias, malformed bootnode, duplicate base path.

Pass one or more `--test-command` values for project-specific checks. Commands run after the network is healthy and the selected built-in suites pass. They receive environment variables describing the network.

```bash
./integration-tests/run-local-network.py \
  --test-command './integration-tests/my-test.sh'
```

Useful environment variables:

- `SORA_INTEGRATION_WORKDIR`
- `SORA_INTEGRATION_CHAIN_SPEC`
- `SORA_INTEGRATION_RPC_URLS`
- `SORA_INTEGRATION_WS_URLS`
- `SORA_INTEGRATION_BOOTNODE_RPC_URL`
- `SORA_INTEGRATION_BOOTNODE_WS_URL`
- `SORA_INTEGRATION_NODE0_RPC_URL`, `SORA_INTEGRATION_NODE1_RPC_URL`, ...
- `SORA_INTEGRATION_NODE0_WS_URL`, `SORA_INTEGRATION_NODE1_WS_URL`, ...
- `SORA_INTEGRATION_NODE0_LOG`, `SORA_INTEGRATION_NODE1_LOG`, ...

## Debug

Keep the network running for manual testing:

```bash
./integration-tests/run-local-network.py --hold --keep-workdir
```

Enable bridge/offchain-worker setup:

```bash
./integration-tests/run-local-network.py --enable-offchain-workers
```

This also inserts local `ethb` keys and copies `misc/eth.json` into each peer base path.
