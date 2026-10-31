# SORA Nexus modification (2026-10-24): Replace Python ECDSA with libsecp256k1; validate BIP32 paths and normalize BIP39 text.
# Python Substrate Interface Library
#
# Copyright 2018-2021 Stichting Polkascan (Polkascan Foundation).
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import hashlib
import hmac
import struct
import unicodedata

from coincurve import PrivateKey as Secp256k1PrivateKey
from eth_keys.backends import CoinCurveECCBackend
from eth_keys.datatypes import Signature, PrivateKey
from eth_utils import to_checksum_address, keccak as eth_utils_keccak

BIP39_PBKDF2_ROUNDS = 2048
BIP39_SALT_MODIFIER = "mnemonic"
BIP32_PRIVDEV = 0x80000000
BIP32_SEED_MODIFIER = b'Bitcoin seed'
ETH_DERIVATION_PATH = "m/44'/60'/0'/0"


class PublicKey:
    def __init__(self, private_key):
        self.public_key = Secp256k1PrivateKey(private_key).public_key

    def __bytes__(self):
        return self.public_key.format(compressed=True)

    def address(self):
        s = self.public_key.format(compressed=False)[1:]
        return to_checksum_address(eth_utils_keccak(s)[12:])


def mnemonic_to_bip39seed(mnemonic, passphrase):
    mnemonic = unicodedata.normalize('NFKD', mnemonic).encode('utf8')
    salt = (BIP39_SALT_MODIFIER + unicodedata.normalize('NFKD', passphrase)).encode('utf8')
    return hashlib.pbkdf2_hmac('sha512', mnemonic, salt, BIP39_PBKDF2_ROUNDS)


def bip39seed_to_bip32masternode(seed):
    h = hmac.new(BIP32_SEED_MODIFIER, seed, hashlib.sha512).digest()
    key, chain_code = h[:32], h[32:]
    # libsecp256k1 validates the master scalar, including zero and >= curve order.
    Secp256k1PrivateKey(key)
    return key, chain_code


def derive_bip32childkey(parent_key, parent_chain_code, i):
    if len(parent_key) != 32 or len(parent_chain_code) != 32:
        raise ValueError('BIP32 private keys and chain codes must be 32 bytes')
    if not 0 <= i <= 0xFFFFFFFF:
        raise ValueError('BIP32 child index must fit in an unsigned 32-bit integer')
    parent = Secp256k1PrivateKey(parent_key)
    k = parent_chain_code
    if (i & BIP32_PRIVDEV) != 0:
        key = b'\x00' + parent_key
    else:
        key = bytes(PublicKey(parent_key))
    key_material = key
    while True:
        d = key_material + struct.pack('>L', i)
        h = hmac.new(k, d, hashlib.sha512).digest()
        try:
            # Do secret scalar arithmetic in libsecp256k1, never Python integers.
            return parent.add(h[:32]).secret, h[32:]
        except ValueError:
            # BIP32 specifies retrying the next index for an invalid child.
            # Do not cross from non-hardened to hardened (or wrap the index).
            if i in (BIP32_PRIVDEV - 1, 0xFFFFFFFF):
                raise ValueError('BIP32 child index range exhausted') from None
            i += 1


def parse_derivation_path(str_derivation_path):
    path = []
    if str_derivation_path == 'm':
        return path
    if str_derivation_path[0:2] != 'm/':
        raise ValueError("Can't recognize derivation path. It should look like \"m/44'/60/0'/0\".")
    for component in str_derivation_path[2:].split('/'):
        hardened = component.endswith("'")
        number = component[:-1] if hardened else component
        if not number.isascii() or not number.isdecimal() or int(number) >= BIP32_PRIVDEV:
            raise ValueError('BIP32 path components must be integers below 2^31, optionally followed by an apostrophe')
        path.append(int(number) + (BIP32_PRIVDEV if hardened else 0))
    return path


def mnemonic_to_ecdsa_private_key(mnemonic: str, str_derivation_path: str = None, passphrase: str = "") -> bytes:

    if str_derivation_path is None:
        str_derivation_path = f'{ETH_DERIVATION_PATH}/0'

    derivation_path = parse_derivation_path(str_derivation_path)
    bip39seed = mnemonic_to_bip39seed(mnemonic, passphrase)
    master_private_key, master_chain_code = bip39seed_to_bip32masternode(bip39seed)
    private_key, chain_code = master_private_key, master_chain_code
    for i in derivation_path:
        private_key, chain_code = derive_bip32childkey(private_key, chain_code, i)
    return private_key


def ecdsa_sign(private_key: bytes, message: bytes) -> bytes:
    signer = PrivateKey(private_key, backend=CoinCurveECCBackend())
    return signer.sign_msg(message).to_bytes()


def ecdsa_verify(signature: bytes, data: bytes, address: bytes) -> bool:
    signature_obj = Signature(signature, backend=CoinCurveECCBackend())
    recovered_pubkey = signature_obj.recover_public_key_from_msg(data)
    return recovered_pubkey.to_canonical_address() == address
