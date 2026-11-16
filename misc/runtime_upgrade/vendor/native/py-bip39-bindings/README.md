# py-bip39-bindings
Python bindings for the tiny-bip39 library: https://crates.io/crates/tiny-bip39

Reference to Reference to https://github.com/LocalCoinSwap/kusama-reference-implementation/tree/improve-trading-tests/bindings for the initial work 

## Documentation

https://docs.rs/py-bip39-bindings/

## Installation

### Install from PyPI

```shell script
pip install py-bip39-bindings
```

### Compile for local development

```shell script
pip install -r requirements.txt
maturin develop
```
### Build wheels
```shell script
pip install -r requirements.txt

# Build local OS wheel
maturin build

# Build manylinux1 wheel
docker build . --tag polkasource/maturin
docker run --rm -i -v $(pwd):/io polkasource/maturin build

```

## Examples

```python
import binascii
from bip39 import bip39_to_mini_secret, bip39_generate, bip39_validate

mnemonic = bip39_generate(12)
bip39_validate(mnemonic)

seed_array = bip39_to_mini_secret(mnemonic, "")
seed_hex = binascii.hexlify(bytearray(seed_array)).decode("ascii")

```

## Multi-language support

The following language codes are supported: 'en', 'zh-hans', 'zh-hant', 'fr', 'it', 'jap', 'ko', 'es'. Defaults to 'en'

```python
mnemonic = bip39_generate(12, 'fr')
# 'moufle veinard tronc magasin merle amour toboggan admettre biotype décembre régalien billard'
bip39_validate(mnemonic, 'fr')

seed_array = bip39_to_mini_secret(mnemonic, "", 'fr')

mnemonic = bip39_generate(12, 'zh-hans')
# '观 敲 荣 硬 责 雪 专 宴 醇 飞 图 菌'
```



## License
https://github.com/polkascan/py-bip39-bindings/blob/master/LICENSE
