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
python main.py --node-url ws://127.0.0.1:9944 --uri //Alice --wasm-file-path /path/to/wasm-file
```

### Using Docker

#### Build Docker Image

```bash
docker build -t runtime-upgrade .
```

#### Run Docker Image

```bash
docker run --rm runtime-upgrade --node-url ws://127.0.0.1:9944 --uri //Alice --wasm-file-path /path/to/wasm-filedoc
```

### Arguments

```
usage: Runtime Upgrade [-h] [--node-url NODE_URL] --wasm-file-path WASM_FILE_PATH
      (--uri URI_KEYPAIR | --seed SEED | --mnemonic MNEMONIC)

options:
-h, --help show this help message and exit
--node-url NODE_URL URL of the node to connect to
--wasm-file-path WASM_FILE_PATH Path to Compressed Wasm File
--uri URI_KEYPAIR URI of the keypair to use
--seed SEED Seed of the keypair to use
--mnemonic MNEMONIC Seed phrase of the keypair to use

```
