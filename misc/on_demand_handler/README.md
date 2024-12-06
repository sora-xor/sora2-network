# On-demand handler Script

Produce SORA Kusama parachain blocks on-demand (in case of bridge transfers and parachain transactions)

## Requirements

- Python 3.11+

  Or

- Docker

## How to use

### Manual Setup

1. Install packages

```bash
pip install -r requirements.txt
```

2. Run script with the specific arguments

```bash
python main.py --sora-node-url ws://127.0.0.1:9944 --parachain-node-url ws://127.0.0.1:9944 --kusama-node-url ws://127.0.0.1:9944 --uri //Alice
```

### Using Docker

#### Build Docker Image

```bash
docker build -t on-demand-handler .
```

#### Run Docker Image

```bash
docker run --rm on-demand-handler --sora-node-url ws://127.0.0.1:9944 --parachain-node-url ws://127.0.0.1:9944 --kusama-node-url ws://127.0.0.1:9944 --uri //Alice 
```

## Arguments

```
usage: On-demand handler [-h] --sora-node-url SORA_NODE_URL --parachain-node-url PARACHAIN_NODE_URL --kusama-node-url KUSAMA_NODE_URL
                         (--uri URI_KEYPAIR | --seed SEED | --mnemonic MNEMONIC)

SORA Parachain bridge on-demand handler to produce parachain blocks

options:
  -h, --help            show this help message and exit
  --sora-node-url SORA_NODE_URL
                        URL of the node to connect to
  --parachain-node-url PARACHAIN_NODE_URL
                        URL of the node to connect to
  --kusama-node-url KUSAMA_NODE_URL
                        URL of the node to connect to
  --uri URI_KEYPAIR     URI of the keypair to use
  --seed SEED           Seed of the keypair to use
  --mnemonic MNEMONIC   Seed phrase of the keypair to use
```
