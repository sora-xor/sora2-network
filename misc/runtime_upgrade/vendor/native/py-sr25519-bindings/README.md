# py-sr25519-bindings
Python bindings for sr25519 library: https://github.com/w3f/schnorrkel

Reference to https://github.com/LocalCoinSwap/kusama-reference-implementation/tree/improve-trading-tests/bindings and https://gitlab.com/kauriid/schnorrpy/ for the initial work 

## Documentation

https://docs.rs/py-sr25519-bindings

## Installation

### Install from PyPI

```
pip install py-sr25519-bindings
```

### Compile for local development

```
pip install -r requirements.txt
maturin develop
```
### Build wheels
```
pip install -r requirements.txt

# Build local OS wheel
maturin build

# Build manylinux1 wheel
docker build . --tag polkasource/maturin
docker run --rm -i -v $(pwd):/io polkasource/maturin build

```

## Usage 

```python
import bip39
import sr25519

message = b"test"

# Get private and public key from seed
seed = bip39.bip39_to_mini_secret('daughter song common combine misery cotton audit morning stuff weasel flee field','')

public_key, private_key = sr25519.pair_from_seed(bytes(seed))

# Generate signature
signature = sr25519.sign(
    (public_key, private_key),
    message
)

print('Signature', signature.hex())

# Verify message with signature
if sr25519.verify(signature, message, public_key):
    print('Verified')
```

## License
https://github.com/polkascan/py-sr25519-bindings/blob/master/LICENSE
