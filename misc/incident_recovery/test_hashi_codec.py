#!/usr/bin/env python3
"""Public native-Rust golden fixtures and fail-closed offline HASHI codec tests."""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import hashi_codec as codec

ROOT = Path(__file__).parent


class HashiCodecTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = codec.read_json(ROOT / "hashi_codec_fixture.json")

    def test_native_rust_scale_and_commitment_match(self):
        fixture = self.golden
        self.assertEqual(codec.commitment(fixture["template"]), fixture["commitment"])
        self.assertEqual("0x" + codec.encode_operation(fixture["template"], fixture["signatures"]).hex(), fixture["operationScale"])

    def test_native_rust_signed_payloads_match(self):
        context = self.golden["nativeContext"]
        for approval, expected in zip(self.golden["template"]["approvals"], self.golden["signingPayloads"]):
            result = codec.native_signing_payload(codec.raw(approval["call"]), codec.raw(approval["extra"]), context["specVersion"], context["transactionVersion"], context["genesisHash"], context["birthHash"])
            self.assertEqual("0x" + result.hex(), expected)

    def test_signature_witnesses_do_not_change_template_commitment(self):
        template = self.golden["template"]
        before = codec.commitment(template)
        signatures = copy.deepcopy(self.golden["signatures"])
        signatures[0] = "0x01" + "00" * 64
        self.assertNotEqual(codec.encode_operation(template, signatures), codec.raw(self.golden["operationScale"]))
        self.assertEqual(codec.commitment(template), before)
        # Assembly deliberately does not claim to authenticate these corrupt witness bytes.

    def test_call_nonce_and_registry_are_committed(self):
        original = self.golden["template"]
        for edit in (lambda t: t["approvals"][0].update(call="0x0102"),
                     lambda t: t["registries"][0].update(attestation="0x" + "22" * 32)):
            changed = copy.deepcopy(original)
            edit(changed)
            self.assertNotEqual(codec.commitment(changed), self.golden["commitment"])
        changed = copy.deepcopy(original)
        extra = codec.raw(changed["approvals"][0]["extra"])
        changed["approvals"][0]["extra"] = "0x" + (extra[:2] + codec.compact(1) + codec.compact(0)).hex()
        self.assertNotEqual(codec.commitment(changed), self.golden["commitment"])

    def test_ambiguous_or_missing_inputs_rejected(self):
        for edit in (lambda t: t.update(network=None),
                     lambda t: t.update(request="0x"),
                     lambda t: t.update(finalizedSourceEvidence="0x" + "00" * 32),
                     lambda t: t["registries"][0].update(accounts=[]),
                     lambda t: t["approvals"][0].update(signature="0x01" + "00" * 64),
                     lambda t: t["approvals"][0].update(extra="0x000000"),
                     lambda t: t["approvals"][0].update(signer=codec.ATTACKER_SORA),
                     lambda t: t["approvals"].append(t["approvals"][0])):
            template = copy.deepcopy(self.golden["template"])
            edit(template)
            with self.assertRaises((ValueError, TypeError)):
                codec.commitment(template)
        with self.assertRaises(ValueError):
            codec.encode_operation(self.golden["template"], [])
        with self.assertRaises(ValueError):
            codec.encode_operation(self.golden["template"], ["0x03" + "00" * 64] * 3)

    def test_compact_and_native_extra_are_canonical(self):
        for value in (0, 63, 64, 16383, 16384, 2**30 - 1, 2**30, 2**64, 2**128 - 1):
            reader = codec.Reader(codec.compact(value))
            self.assertEqual(reader.compact(), value)
            reader.end()
        with self.assertRaises(ValueError):
            codec.Reader(bytes([1, 0])).compact()
        extra = codec.raw(self.golden["template"]["approvals"][0]["extra"])
        for suffix in (codec.compact(1), b"\x00\x00"):
            with self.assertRaises(ValueError):
                codec.decode_extra(extra[:2] + codec.compact(0) + suffix)

    def source_template(self):
        template = copy.deepcopy(self.golden["template"])
        template["format"] = codec.SOURCE_FORMAT
        request = codec.raw(template["request"])
        template["request"] = "0x" + (request[:1] + bytes(20) + request[21:]).hex()
        template["rawTransaction"] = "0x0102"
        template["expectedChainId"] = "0x" + "00" * 31 + "01"
        return template

    def test_source_domain_and_envelope_are_bound_without_claiming_crypto(self):
        template = self.source_template()
        encoded = codec.encode_operation(template, self.golden["signatures"])
        self.assertEqual(encoded[4], 3)
        reader = codec.Reader(encoded[5:])
        self.assertEqual(reader.take(4), codec.u32(template["network"]))
        self.assertEqual(reader.vector(), codec.raw(template["request"]))
        self.assertEqual(reader.vector(), b"\x01\x02")
        self.assertEqual(reader.take(33), b"\x01\x01" + bytes(31))
        before = codec.commitment(template)
        for edit in (lambda t: t.update(rawTransaction="0x0103"),
                     lambda t: t.update(expectedChainId=None),
                     lambda t: t.update(expectedChainId="0x" + "00" * 31 + "02")):
            changed = copy.deepcopy(template)
            edit(changed)
            self.assertNotEqual(codec.commitment(changed), before)
        changed_signatures = copy.deepcopy(self.golden["signatures"])
        changed_signatures[0] = "0x01" + "00" * 64
        self.assertNotEqual(codec.encode_operation(template, changed_signatures), encoded)
        self.assertEqual(codec.commitment(template), before)

    def test_source_bounds_native_sender_and_explicit_domain_rejected(self):
        for edit in (lambda t: t.update(rawTransaction="0x"),
                     lambda t: t.update(rawTransaction="0x" + "01" * (codec.MAX_SIGNED_TRANSACTION_BYTES + 1)),
                     lambda t: t.update(expectedChainId=1),
                     lambda t: t.update(expectedChainId="0x01"),
                     lambda t: t.update(request=self.golden["template"]["request"]),
                     lambda t: t.pop("expectedChainId")):
            changed = self.source_template()
            edit(changed)
            with self.assertRaises((ValueError, TypeError)):
                codec.commitment(changed)

    @unittest.skipUnless(os.environ.get("SORA_RECOVERY_HASHI_SOURCE_GOLDEN_PATH"), "requires actual native exported new-variant golden")
    def test_source_actual_native_rust_golden(self):
        fixture = codec.read_json(Path(os.environ["SORA_RECOVERY_HASHI_SOURCE_GOLDEN_PATH"]))
        self.assertEqual(codec.commitment(fixture["template"]), fixture["commitment"])
        self.assertEqual("0x" + codec.encode_operation(fixture["template"], fixture["signatures"]).hex(), fixture["operationScale"])

    def lineage_context(self):
        policy = "0x" + "33" * 32
        marker = codec.MAGIC + codec.raw(codec.CHECKPOINT_HASH) + codec.raw(policy)
        header = codec.raw(codec.CHECKPOINT_HASH) + codec.compact(codec.CHECKPOINT + 1) + bytes(64) + codec.compact(1) + b"\x00" + codec.vec(marker)
        head = "0x" + hashlib.blake2b(header, digest_size=32).hexdigest()
        return {"specVersion": 134, "transactionVersion": 131,
                "genesisHash": codec.GENESIS_HASH,
                "executionCodeHash": "0x" + "55" * 32, "metadataHash": "0x" + "66" * 32,
                "executionBlock": codec.CHECKPOINT + 2,
                "lineage": {"policyHash": policy, "headers": ["0x" + header.hex()],
                            "expectedFinalizedHeadHash": head, "finalityEvidenceSha256": "0x" + "77" * 32}}

    def test_post_fork_header_ancestry_required_for_signing(self):
        context = self.lineage_context()
        result = codec.signing_payloads(self.golden["template"], context)
        self.assertEqual(result["payloads"][0]["birthBlock"], codec.CHECKPOINT + 1)
        self.assertIn("GRANDPA finality requires", result["lineageVerification"])
        for edit in (lambda c: c["lineage"].update(expectedFinalizedHeadHash="0x" + "11" * 32),
                     lambda c: c["lineage"].update(policyHash="0x" + "12" * 32),
                     lambda c: c["lineage"].update(headers=[]),
                     lambda c: c.update(executionBlock=codec.CHECKPOINT + 1),
                     lambda c: c.update(genesisHash="0x" + "13" * 32)):
            changed = copy.deepcopy(context)
            edit(changed)
            with self.assertRaises(ValueError):
                codec.signing_payloads(self.golden["template"], changed)

    def test_native_256_byte_hash_threshold(self):
        extra = codec.raw(self.golden["template"]["approvals"][0]["extra"])
        context = self.golden["nativeContext"]
        suffix = extra + codec.u32(context["specVersion"]) + codec.u32(context["transactionVersion"]) + codec.raw(context["genesisHash"]) + codec.raw(context["birthHash"])
        for total in (256, 257):
            call = bytes(total - len(suffix))
            actual = codec.native_signing_payload(call, extra, context["specVersion"], context["transactionVersion"], context["genesisHash"], context["birthHash"])
            expected = call + suffix if total == 256 else hashlib.blake2b(call + suffix, digest_size=32).digest()
            self.assertEqual(actual, expected)

    def test_cli_exclusive_output_and_duplicate_json_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            template = Path(directory) / "template.json"
            output = Path(directory) / "output.json"
            template.write_text(json.dumps(self.golden["template"]))
            command = [sys.executable, str(ROOT / "hashi_codec.py"), "prepare", "--template", str(template), "--output", str(output)]
            self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
            previous = output.read_bytes()
            self.assertNotEqual(subprocess.run(command, capture_output=True).returncode, 0)
            self.assertEqual(output.read_bytes(), previous)
            template.write_text('{"format":1,"format":2}')
            with self.assertRaises(ValueError):
                codec.read_json(template)


if __name__ == "__main__":
    unittest.main()
