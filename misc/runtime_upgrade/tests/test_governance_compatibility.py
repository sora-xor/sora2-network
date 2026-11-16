"""Encode real archived runtime metadata entirely offline; never submit anything."""

import hashlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

from scalecodec.base import ScaleBytes

TOOL_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOL_DIR))
import main as runtime_upgrade


class GovernanceCompatibilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = json.loads((TOOL_DIR / "tests/fixtures/governance.json").read_text())
        metadata_path = TOOL_DIR.parents[1] / cls.fixtures["metadata_path"]
        metadata_bytes = metadata_path.read_bytes()
        if hashlib.sha256(metadata_bytes).hexdigest() != cls.fixtures["metadata_sha256"]:
            raise AssertionError("Archived metadata fixture checksum changed")
        cls.substrate = runtime_upgrade.SubstrateInterface(
            websocket=Mock(), ss58_format=69, auto_discover=False
        )
        cls.substrate.rpc_request = Mock(side_effect=AssertionError("Network access forbidden"))
        metadata = cls.substrate.runtime_config.create_scale_object(
            "MetadataVersioned", data=ScaleBytes(metadata_bytes)
        )
        metadata.decode()
        cls.substrate.metadata = metadata
        cls.substrate.runtime_config.add_portable_registry(metadata)
        cls.substrate.runtime_version = 132
        cls.substrate.transaction_version = 1
        cls.substrate.runtime_config.set_active_spec_version_id(132)
        cls.substrate.init_runtime = Mock()
        cls.substrate.get_block_hash = Mock(return_value="0x" + "00" * 32)
        cls.keypair = runtime_upgrade.Keypair.create_from_uri("//Alice", ss58_format=69)

    @classmethod
    def tearDownClass(cls):
        cls.substrate.close()

    def test_governance_call_payload_and_extrinsic_bytes_match_upstream(self):
        substrate = self.substrate
        proposal_hash = "0x" + "12" * 32
        calls = {
            "council": runtime_upgrade.compose_council_motion(substrate, proposal_hash, 123, 3),
            "tech": runtime_upgrade.compose_fast_track_motion(substrate, proposal_hash, 3, 10, 0),
        }
        with tempfile.NamedTemporaryFile() as sample:
            sample.write(b"\x00asm\x01\x00\x00\x00")
            sample.flush()
            calls["note"] = runtime_upgrade.handle_note_preimage(
                substrate, SimpleNamespace(call_file_path=sample.name)
            )
        for name, call in calls.items():
            with self.subTest(command=name):
                expected = self.fixtures["calls"][name]
                payload = substrate.generate_signature_payload(call=call, nonce=0)
                extrinsic = substrate.create_signed_extrinsic(
                    call=call, keypair=self.keypair, nonce=0, signature=b"\x01" * 64
                )
                self.assertEqual(str(call.data), expected["call"])
                self.assertEqual(str(payload), expected["payload"])
                self.assertEqual(str(extrinsic.data), expected["extrinsic"])
                signature = self.keypair.sign(payload)
                self.assertTrue(self.keypair.verify(payload, signature))
        substrate.rpc_request.assert_not_called()

    def test_cli_authentication_forms_retain_sr25519(self):
        fixtures = json.loads((TOOL_DIR / "tests/fixtures/keypairs.json").read_text())
        for fixture in fixtures:
            if fixture["crypto_type"] != 1:
                continue
            args = SimpleNamespace(uri_keypair=None, seed=None, mnemonic=None)
            setattr(args, "uri_keypair" if fixture["method"] == "uri" else fixture["method"], fixture["input"])
            keypair = runtime_upgrade.get_keypair_using_args(args)
            self.assertEqual(keypair.crypto_type, 1)
            self.assertEqual(keypair.ss58_address, fixture["address"])

    def test_sudo_unchecked_preserves_storage_reset_and_batch(self):
        # Production metadata deliberately has no Sudo pallet; check the dev-only API composition.
        substrate = Mock()
        substrate.generate_storage_hash.return_value = "0x1234"
        with tempfile.NamedTemporaryFile() as wasm:
            wasm.write(b"\x00asm")
            wasm.flush()
            runtime_upgrade.handle_sudo_set_code(
                substrate, SimpleNamespace(wasm_file_path=wasm.name, unchecked=True)
            )
        calls = substrate.compose_call.call_args_list
        self.assertEqual([c.kwargs["call_module"] for c in calls], ["System", "System", "Utility", "Sudo"])
        self.assertEqual([c.kwargs["call_function"] for c in calls],
                         ["set_code_without_checks", "kill_storage", "batch_all", "sudo_unchecked_weight"])
        self.assertEqual(calls[1].kwargs["call_params"]["keys"], ["0x1234"])


if __name__ == "__main__":
    unittest.main()
