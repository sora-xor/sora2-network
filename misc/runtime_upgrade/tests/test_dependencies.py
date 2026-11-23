"""Offline regression coverage for the runtime-upgrade cryptography migration."""

import importlib.util
import json
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import Mock, patch

import base58
from coincurve.utils import GROUP_ORDER_INT
from eth_keys.backends import NativeECCBackend

TOOL_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOL_DIR))

from vendor.substrateinterface import Keypair, KeypairType, SubstrateInterface
from vendor.substrateinterface.exceptions import ConfigurationError
from vendor.substrateinterface.utils import ecdsa_helpers as crypto


class DependencyMigrationTests(unittest.TestCase):
    def test_vulnerable_distribution_is_absent(self):
        self.assertIsNone(importlib.util.find_spec("ecdsa"))
        self.assertIsNone(importlib.util.find_spec("substrateinterface"))
        self.assertIsNone(importlib.util.find_spec("smoldot_light"))

    def test_light_client_is_rejected_without_loading_a_native_backend(self):
        # Even a stale or separately installed module must not activate the
        # vulnerable native transport or cause a chainspec file to be read.
        with patch.dict(sys.modules, {"smoldot_light": Mock()}):
            with self.assertRaisesRegex(ConfigurationError, "HTTP or WebSocket RPC URL"):
                SubstrateInterface(chainspec="unread-chainspec.json")

    def test_bip32_official_vectors(self):
        fixtures = json.loads((TOOL_DIR / "tests/fixtures/bip32.json").read_text())
        for vector in fixtures["vectors"]:
            master = crypto.bip39seed_to_bip32masternode(bytes.fromhex(vector["seed"]))
            for child in vector["children"]:
                with self.subTest(seed=vector["seed"], path=child["path"]):
                    key, chain = master
                    for index in crypto.parse_derivation_path(child["path"]):
                        key, chain = crypto.derive_bip32childkey(key, chain, index)
                    private = base58.b58decode_check(child["private"])
                    public = base58.b58decode_check(child["public"])
                    self.assertEqual(key, private[-32:])
                    self.assertEqual(chain, private[13:45])
                    self.assertEqual(bytes(crypto.PublicKey(key)), public[-33:])

    def test_keypair_compatibility_with_upstream(self):
        fixtures = json.loads((TOOL_DIR / "tests/fixtures/keypairs.json").read_text())
        for fixture in fixtures:
            with self.subTest(method=fixture["method"], scheme=fixture["crypto_type"]):
                keypair = getattr(Keypair, "create_from_" + fixture["method"])(
                    fixture["input"], crypto_type=fixture["crypto_type"], ss58_format=69
                )
                self.assertEqual(keypair.public_key.hex(), fixture["public_key"])
                self.assertEqual(keypair.ss58_address, fixture["address"])
                message = bytes.fromhex(fixture["message"])
                self.assertTrue(keypair.verify(message, bytes.fromhex(fixture["signature"])))
                signature = keypair.sign(message)
                self.assertTrue(keypair.verify(message, signature))
                self.assertFalse(keypair.verify(message + b"tampered", signature))
                if fixture["crypto_type"] != KeypairType.SR25519:
                    # SR25519 signatures are randomized; the other schemes are deterministic.
                    self.assertEqual(signature.hex(), fixture["signature"])

    def test_ecdsa_cannot_fall_back_to_python_backend(self):
        with patch.object(NativeECCBackend, "ecdsa_sign", side_effect=AssertionError("Python ECDSA used")):
            keypair = Keypair.create_from_private_key("0x" + "01" * 32, crypto_type=KeypairType.ECDSA)
            signature = keypair.sign(b"test libsecp256k1")
            self.assertTrue(keypair.verify(b"test libsecp256k1", signature))

    def test_bip39_known_seed_and_unicode_normalization(self):
        mnemonic = "abandon " * 11 + "about"
        self.assertEqual(
            crypto.mnemonic_to_bip39seed(mnemonic, "TREZOR").hex(),
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531"
            "f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04",
        )
        self.assertEqual(
            crypto.mnemonic_to_bip39seed(mnemonic, "caf\u00e9"),
            crypto.mnemonic_to_bip39seed(mnemonic, "cafe\u0301"),
        )

    def test_invalid_child_retries_next_index(self):
        parent = (1).to_bytes(32, "big")
        chain = b"c" * 32
        for invalid in (GROUP_ORDER_INT, GROUP_ORDER_INT - 1):
            # First tweak is invalid or produces zero; the next tweak produces key 2.
            with self.subTest(tweak=invalid):
                responses = [Mock(digest=lambda: invalid.to_bytes(32, "big") + b"x" * 32),
                             Mock(digest=lambda: parent + b"y" * 32)]
                with patch.object(crypto.hmac, "new", side_effect=responses) as hmac:
                    key, child_chain = crypto.derive_bip32childkey(parent, chain, 7)
                self.assertEqual(key, (2).to_bytes(32, "big"))
                self.assertEqual(child_chain, b"y" * 32)
                self.assertEqual(hmac.call_args_list[0].args[1][-4:], struct.pack(">L", 7))
                self.assertEqual(hmac.call_args_list[1].args[1][-4:], struct.pack(">L", 8))

    def test_invalid_child_cannot_cross_derivation_boundary(self):
        invalid = Mock(digest=lambda: GROUP_ORDER_INT.to_bytes(32, "big") + b"x" * 32)
        for index in (0x7FFFFFFF, 0xFFFFFFFF):
            with self.subTest(index=index), patch.object(crypto.hmac, "new", return_value=invalid):
                with self.assertRaisesRegex(ValueError, "range exhausted"):
                    crypto.derive_bip32childkey((1).to_bytes(32, "big"), b"c" * 32, index)

    def test_invalid_master_and_derivation_inputs_fail(self):
        for value in (0, GROUP_ORDER_INT):
            digest = value.to_bytes(32, "big") + b"c" * 32
            with patch.object(crypto.hmac, "new", return_value=Mock(digest=lambda: digest)):
                with self.assertRaises(ValueError):
                    crypto.bip39seed_to_bip32masternode(b"invalid master")
        for path in ("", "m/", "m/-1", "m/2147483648", "m/2147483648'", "m/1''", "m/1/", "m/\u0661"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                crypto.parse_derivation_path(path)
        for index in (-1, 2**32):
            with self.assertRaises(ValueError):
                crypto.derive_bip32childkey(b"\x01" * 32, b"c" * 32, index)


if __name__ == "__main__":
    unittest.main()
