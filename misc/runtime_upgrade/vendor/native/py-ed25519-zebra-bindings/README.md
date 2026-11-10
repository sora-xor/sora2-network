# py-ed25519-zebra-bindings
Python bindings for the [ed25519-zebra RUST crate](https://crates.io/crates/ed25519-dalek/3.0.0) 

## Documentation

https://docs.rs/py-ed25519-zebra-bindings

## Installation

### Install from PyPI

```
pip install py-ed25519-zebra-bindings
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
maturin build --release

# Build manylinux1 wheel
docker run --rm -v $(pwd):/io ghcr.io/pyo3/maturin build --release

```

## Usage

```python
import bip39
import ed25519_zebra

message = b"test"

# Get private and public key from seed
seed = bip39.bip39_to_mini_secret('daughter song common combine misery cotton audit morning stuff weasel flee field','')
private_key, public_key = ed25519_zebra.ed_from_seed(bytes(seed))

# Generate signature
signature = ed25519_zebra.ed_sign(private_key, message)

# Verify message with signature
if ed25519_zebra.ed_verify(signature, message, public_key):
    print('Verified')

```


## License
https://github.com/polkascan/py-ed25519-zebra-bindings/blob/master/LICENSE
