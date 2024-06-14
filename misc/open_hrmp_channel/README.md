# Runtime Upgrade Script

Upgrade Runtime of a Substrate node

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
python main.py --node-url ws://127.0.0.1:9944 --uri //Alice --first-para-id 2011 --second-para-id 1000
```

### Using Docker

#### Build Docker Image

```bash
docker build -t runtime-upgrade .
```

#### Run Docker Image

```bash
docker run --rm -v /host/path/to/wasm-file:/container/path/to/wasm-file runtime-upgrade --node-url ws://127.0.0.1:9944 --uri //Alice --first-para-id 2011 --second-para-id 1000
```

## Arguments

```
usage: Open HRMP Channel [-h] [--node-url NODE_URL] --first-para-id FIRST_PARA_ID --second-para-id SECOND_PARA_ID [--capacity CAPACITY] [--message-size MESSAGE_SIZE]
                         (--uri URI_KEYPAIR | --seed SEED | --mnemonic MNEMONIC)

Open HRMP channel between parachains

options:
  -h, --help            show this help message and exit
  --node-url NODE_URL   URL of the relaychain node
  --first-para-id FIRST_PARA_ID
                        First para ID
  --second-para-id SECOND_PARA_ID
                        Second para ID
  --capacity CAPACITY   Channel capacity (default 4)
  --message-size MESSAGE_SIZE
                        Channel max message size (default 524287)
  --uri URI_KEYPAIR     URI of the keypair to use
  --seed SEED           Seed of the keypair to use
  --mnemonic MNEMONIC   Seed phrase of the keypair to use

```
