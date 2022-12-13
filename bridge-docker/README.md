## Run bridge in docker-compose

### Run

```
docker-compose up -d --build
```

### Stop

```
docker-compose down
```

### Stop and remove volumes

```
docker-compose down -v
```

### Update cached dependencies

```
cargo chef prepare --recipe-path bridge-docker/recipe.json
```


## Accounts

### Relayer

**Address:** `0xd07FF88cB22399F9A50A1A9173a939e163e8F541`

**Private key**: `3b61c8157aea9aba36248468af274cac4163b0b58c63eb66a8d2bbf219906c62`

### Deployer

**Address:** `0xa66C22009dc2DaC73f0730dA9015C679c0ec372C`

**Private key:** `21754896455c7e745e7f14d4f7782bbdf7769a0539b2fe8682fa0a2e13f37075`

### Faucet

**Address:** `0xD6489E039b0eF70698CA06c8ce77Bcd0e7aE9a85`

**Private key:** `5e5d0ada9dbe15b601d119b076a792eacc828470c6304037c69cfba397a94e41`

## Services

**Blockscout:** http://localhost:4000

**Geth endpoint:** http://localhost:8545

**Sora Alice:** http://localhost:9944

**Sora Bob:** http://localhost:9945

**Sora Charlie:** http://localhost:9946

**Sora Dave:** http://localhost:9947

**Sora Eve:** http://localhost:9948

**Sora Ferdie:** http://localhost:9949

**PG Web:** http://localhost:8081