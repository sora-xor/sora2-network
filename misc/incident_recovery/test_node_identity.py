"""Synthetic tests only: no native node executable is run by this suite."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import node_identity as n
from test_release_package import policy


def ss58_fixture(raw, prefix=42):
    """Test-only public-key encoding; no native executable or signing key."""
    body = bytes([prefix]) + bytes.fromhex(raw.removeprefix("0x"))
    data = body + hashlib.blake2b(b"SS58PRE" + body, digest_size=64).digest()[:2]
    number, result = int.from_bytes(data, "big"), ""
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    while number:
        number, index = divmod(number, 58)
        result = alphabet[index] + result
    return "1" * (len(data) - len(data.lstrip(b"\0"))) + result


class Fixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.manifest = self.root / "manifest.json"
        self.output = self.root / "execution"
        self.wasm = b"\0asm\x01\0\0\0"
        h = lambda byte: "0x" + byte * 64
        self.config = {"schemaVersion": 3, "policyScale": "0x" + policy().hex(), "checkpointHash": n.CHECKPOINT_HASH,
                       "policyCommitment": "0x" + hashlib.blake2b(policy(), digest_size=32).hexdigest(), "bootstrapCodeHash": "0x" + hashlib.blake2b(self.wasm, digest_size=32).hexdigest(),
                       "holdCodeHash": h("2"), "finalCodeHash": h("3"), "evidenceCommitment": "0x" + (bytes([6]) * 32).hex(),
                       "previousGrandpaSetId": 7956, "previousGrandpaAuthorities": [[ss58_fixture(h("5")), 1]],
                       "grandpaAuthorities": [[ss58_fixture(h("5")), 1]],
                       "approvedGrandpaKeys": [ss58_fixture(h("5"))], "lastObservedAbandonedSlot": 298871044,
                       "firstRecoverySlot": 298871045}
        self.spec = {"name": "SORA Nexus synthetic offline template", "id": "sora-nexus-recovery-28191817",
                     "protocolId": "sora-nexus-recovery-28191817-v1", "forkId": "recovery-28191817-v1",
                     "recovery": self.config, "bootNodes": [], "telemetryEndpoints": [],
                     "genesis": {"raw": {"top": {"0x3a636f6465": "0x00"}, "childrenDefault": {}}},
                     "codeSubstitutes": {"28191817": "0x" + self.wasm.hex()}}
        self.binary = self.root / "node"
        self.binary.write_bytes(b"\x7fELFsynthetic-not-a-real-executable")
        self.binary.chmod(0o500)
        (self.root / "bootstrap.wasm").write_bytes(self.wasm)
        self.refresh()
        self.calls = []
        self.version = b"framenode 4.8.13-123abcd-aarch64-apple-darwin\n\n"
        self.generated = None

    def tearDown(self):
        self.temp.cleanup()

    def refresh(self):
        (self.root / "config.json").write_text(json.dumps(self.config))
        (self.root / "spec.json").write_text(json.dumps(self.spec))
        names = {"nodeBinary": "node", "chainSpec": "spec.json", "recoveryConfig": "config.json",
                 "bootstrapWasm": "bootstrap.wasm"}
        self.input = {"schema": n.SCHEMA, "artifacts": {role: {"file": file,
                      **n.file_pin(self.root / file, n.ROLES[role])} for role, file in names.items()}}
        self.manifest.write_text(json.dumps(self.input))

    def fake_run(self, argv, output, label, environment, timeout):
        self.calls.append((argv, label, environment, timeout))
        if label == "version":
            data = self.version
        else:
            self.assertEqual(label, "prepare")
            Path(argv[-1]).write_text(json.dumps(self.spec if self.generated is None else self.generated))
            data = b""
        return {"syntheticTestOnly": True, "argv": argv}, data

    def verify(self):
        with patch.object(n, "run_fixed", side_effect=self.fake_run):
            return n.verify(self.manifest, self.output)

    def valid_control(self):
        result = self.verify()
        self.assertTrue(result["exactOfflineTemplateReproduced"])
        self.output = self.root / "negative"
        self.calls.clear()
        return result

    def test_fixed_generous_timeout_budgets(self):
        self.verify()
        self.assertEqual(n.VERSION_TIMEOUT_SECONDS, 900)
        self.assertEqual(n.PREPARE_TIMEOUT_SECONDS, 3600)
        self.assertEqual([(call[1], call[3]) for call in self.calls],
                         [("version", n.VERSION_TIMEOUT_SECONDS),
                          ("prepare", n.PREPARE_TIMEOUT_SECONDS)])

    def test_positive_fixed_commands_and_honest_scope(self):
        with patch.dict(os.environ, {"DYLD_INSERT_LIBRARIES": "forbidden", "HTTP_PROXY": "forbidden"}):
            report = self.verify()
        self.assertEqual(self.calls[0][0], [str(self.output / "framenode"), "--version"])
        self.assertEqual(self.calls[1][0], [str(self.output / "framenode"), "prepare-recovery-spec",
                         "--recovery-config", str(self.output / "recovery-config.json"),
                         "--bootstrap-wasm", str(self.output / "bootstrap.wasm"),
                         "--output", str(self.output / "prepared.json")])
        self.assertEqual(report["versionStdoutSha256"], n.sha(self.version))
        self.assertEqual(report["artifacts"]["nodeBinary"], n.file_pin(self.binary, n.ROLES["nodeBinary"]))
        for field in ("productionReady", "releaseGatePromoted", "databaseStartupVerified", "consensusReadinessVerified",
                      "productionNetworkAdmissionVerified", "nodeSourceBuildAuthenticityVerified", "operatorInventoriesApproved",
                      "noNetworkSandboxEnforced"):
            self.assertIs(report[field], False)
        self.assertEqual(self.calls[0][2], self.calls[1][2])
        self.assertNotIn("HTTP_PROXY", self.calls[0][2])
        self.assertNotIn("DYLD_INSERT_LIBRARIES", self.calls[0][2])
        self.assertNotIn("HOME", self.calls[0][2])
        self.assertNotIn("CODEX_HOME", self.calls[0][2])

    def compressed(self, raw=None):
        from compression import zstd
        return n.SDK_ZSTD_PREFIX + zstd.compress(self.wasm if raw is None else raw)

    def install_bootstrap(self, code):
        (self.root / "bootstrap.wasm").write_bytes(code)
        self.config["bootstrapCodeHash"] = "0x" + hashlib.blake2b(code, digest_size=32).hexdigest()
        self.spec["codeSubstitutes"]["28191817"] = "0x" + code.hex()
        self.refresh()

    def test_compressed_bytes_remain_exact_commitment_and_substitute(self):
        raw = self.wasm
        compressed = self.compressed()
        self.install_bootstrap(compressed)
        result = self.valid_control()
        self.assertEqual(result["bootstrapCodeHash"], "0x" + hashlib.blake2b(compressed, digest_size=32).hexdigest())
        self.assertEqual((self.root / "execution/bootstrap.wasm").read_bytes(), compressed)
        self.config["bootstrapCodeHash"] = "0x" + hashlib.blake2b(raw, digest_size=32).hexdigest()
        self.refresh()
        with self.assertRaisesRegex(ValueError, "bootstrap code hash mismatch"):
            self.verify()
        self.assertEqual(self.calls, [])

    def test_compressed_substitute_cannot_use_decompressed_bytes(self):
        self.install_bootstrap(self.compressed())
        self.valid_control()
        self.spec["codeSubstitutes"]["28191817"] = "0x" + self.wasm.hex()
        self.refresh()
        with self.assertRaisesRegex(ValueError, "checkpoint substitute bytes mismatch"):
            self.verify()
        self.assertEqual(self.calls, [])

    def test_compressed_malformed_truncated_and_tail_rejected(self):
        complete = self.compressed()
        n.validate_bootstrap(complete)
        for value, reason in ((n.SDK_ZSTD_PREFIX + b"garbage", "invalid SDK-compressed"),
                              (complete[:-1], "one complete frame"),
                              (complete + b"tail", "one complete frame"),
                              (complete + complete[8:], "one complete frame"),
                              (self.compressed(b"not wasm"), "decode to a Wasm")):
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, reason):
                n.validate_bootstrap(value)

    def test_sdk_streaming_frame_window_is_bounded(self):
        from compression import zstd
        compressor = zstd.ZstdCompressor()
        frame = bytearray(compressor.compress(self.wasm) + compressor.flush())
        self.assertEqual(frame[:5], bytes.fromhex("28b52ffd00"))
        frame[5] = 0x88  # SDK level22 frame window:128MiB, unknown content size.
        n.validate_bootstrap(n.SDK_ZSTD_PREFIX + bytes(frame))
        frame[5] = 0x90  #256MiB must fail before allocating that decoder window.
        with self.assertRaisesRegex(ValueError, "invalid SDK-compressed bootstrap"):
            n.validate_bootstrap(n.SDK_ZSTD_PREFIX + bytes(frame))

    def test_raw_and_decompressed_size_limits(self):
        n.validate_bootstrap(self.wasm)
        oversized = self.wasm + b"x" * 121
        compressed = self.compressed(oversized)
        with patch.object(n, "MAX_WASM_BYTES", 128):
            n.validate_bootstrap(self.compressed(self.wasm + b"x" * 120))
            with self.assertRaisesRegex(ValueError, "bootstrap Wasm size bound"):
                n.validate_bootstrap(oversized)
            with self.assertRaisesRegex(ValueError, "decompressed bootstrap Wasm size bound"):
                n.validate_bootstrap(compressed)

    def test_known_sdk_ss58_key(self):
        raw = "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
        address = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
        self.assertEqual(ss58_fixture(raw), address)
        self.assertEqual(n.authority_key(address), bytes.fromhex(raw[2:]))

    def test_hex_authority_keys_rejected_before_mock_command(self):
        self.valid_control()
        for name in ("previousGrandpaAuthorities", "grandpaAuthorities", "approvedGrandpaKeys"):
            saved = copy.deepcopy(self.config[name])
            if name.endswith("Authorities"):
                self.config[name][0][0] = "0x" + "55" * 32
            else:
                self.config[name][0] = "0x" + "55" * 32
            self.refresh()
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "canonical SS58 prefix42"):
                self.verify()
            self.assertEqual(self.calls, [])
            self.config[name] = saved

    def test_alternate_prefix_and_bad_checksum_rejected(self):
        self.valid_control()
        correct = self.config["approvedGrandpaKeys"][0]
        malformed = correct[:-1] + ("1" if correct[-1] != "1" else "2")
        for value, reason in ((ss58_fixture("66" * 32, 0), "canonical SS58 prefix42"),
                              (malformed, "checksum mismatch")):
            self.config["approvedGrandpaKeys"][0] = value
            self.refresh()
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, reason):
                self.verify()
            self.assertEqual(self.calls, [])

    def test_authority_json_shapes_rejected(self):
        self.valid_control()
        original = copy.deepcopy(self.config)
        for name, value, reason in (("grandpaAuthorities", [[ss58_fixture("66" * 32), True]], "key/weight pair"),
                                    ("grandpaAuthorities", [[ss58_fixture("66" * 32), 2**64]], "key/weight pair"),
                                    ("approvedGrandpaKeys", "invalid", "bounded GRANDPA key")):
            self.config[name] = value
            self.refresh()
            with self.subTest(name=name, value=value), self.assertRaisesRegex(ValueError, reason):
                self.verify()
            self.assertEqual(self.calls, [])
            self.config.clear()
            self.config.update(copy.deepcopy(original))

    def test_retained_consensus_and_public_slot_policy_before_commands(self):
        self.valid_control()
        original = copy.deepcopy(self.config)
        cases = [('schemaVersion', 2), ('previousGrandpaSetId', 7957),
                 ('grandpaAuthorities', [[ss58_fixture('66' * 32), 1]]),
                 ('grandpaAuthorities', [[ss58_fixture('55' * 32), 2]]),
                 ('approvedGrandpaKeys', [ss58_fixture('66' * 32)]),
                 ('approvedGrandpaKeys', [ss58_fixture('55' * 32)] * 2),
                 ('lastObservedAbandonedSlot', 298871043), ('lastObservedAbandonedSlot', True),
                 ('firstRecoverySlot', 298871044), ('firstRecoverySlot', 2**64),
                 ('retiredGrandpaKeys', []), ('lastAbandonedSignedSlot', 298871044)]
        for name, value in cases:
            self.config.clear(); self.config.update(copy.deepcopy(original)); self.config[name] = value
            self.refresh()
            with self.subTest(name=name, value=value), self.assertRaises(ValueError): self.verify()
            self.assertEqual(self.calls, [])

    def test_wrong_version_fails_before_prepare(self):
        self.valid_control()
        self.version = b"framenode 4.8.12-123-aarch64-apple-darwin\n"
        with self.assertRaisesRegex(ValueError, "version stdout"):
            self.verify()
        self.assertEqual(len(self.calls), 1)
        self.assertFalse((self.output / "result.json").exists())
        self.assertIn("version stdout", (self.output / "failure.json").read_text())

    def test_extra_version_lines_rejected(self):
        self.valid_control()
        self.version += b"productionReady=true\n"
        with self.assertRaisesRegex(ValueError, "version stdout"):
            self.verify()

    def test_generated_spec_mismatch(self):
        self.valid_control()
        self.generated = copy.deepcopy(self.spec)
        self.generated["genesis"]["raw"]["top"]["0x3a636f6465"] = "0x01"
        with self.assertRaisesRegex(ValueError, "prepared template differs"):
            self.verify()

    def test_spec_equality_is_type_sensitive(self):
        self.valid_control()
        self.generated = copy.deepcopy(self.spec)
        self.generated["recovery"]["schemaVersion"] = True
        with self.assertRaisesRegex(ValueError, "prepared template differs"):
            self.verify()

    def test_endpoints_are_never_ignored(self):
        self.valid_control()
        self.spec["bootNodes"] = ["/ip4/127.0.0.1/tcp/30333"]
        self.refresh()
        with self.assertRaisesRegex(ValueError, "empty discovery"):
            self.verify()
        self.assertFalse(self.calls)

    def test_inputs_are_rechecked_after_command(self):
        self.valid_control()
        def mutate(*args):
            result = self.fake_run(*args)
            self.binary.chmod(0o600)
            self.binary.write_bytes(b"\x7fELFchanged")
            return result
        with patch.object(n, "run_fixed", side_effect=mutate), self.assertRaisesRegex(ValueError, "artifact changed"):
            n.verify(self.manifest, self.output)

    def test_copied_binary_is_rechecked(self):
        self.valid_control()
        def mutate(*args):
            result = self.fake_run(*args)
            target = self.output / "framenode"
            target.chmod(0o600)
            target.write_bytes(b"\x7fELFchanged")
            return result
        with patch.object(n, "run_fixed", side_effect=mutate), self.assertRaisesRegex(ValueError, "artifact changed"):
            n.verify(self.manifest, self.output)

    def test_no_arbitrary_commands_or_pass_flags(self):
        self.valid_control()
        self.input["command"] = ["echo", "pass"]
        self.manifest.write_text(json.dumps(self.input))
        with self.assertRaisesRegex(ValueError, "unknown or missing fields"):
            self.verify()
        self.assertFalse(self.calls)

    def test_unpinned_binary_rejected(self):
        self.valid_control()
        self.input["artifacts"]["nodeBinary"]["sha256"] = "0" * 64
        self.manifest.write_text(json.dumps(self.input))
        with self.assertRaisesRegex(ValueError, "binding mismatch: nodeBinary"):
            self.verify()

    def test_scripts_are_rejected(self):
        self.valid_control()
        self.binary.chmod(0o700)
        self.binary.write_bytes(b"#!/bin/sh\necho spoof\n")
        self.refresh()
        with self.assertRaisesRegex(ValueError, "native ELF or Mach-O"):
            self.verify()

    def test_existing_output_preserved(self):
        self.verify()
        before = (self.output / "result.json").read_bytes()
        with self.assertRaisesRegex(ValueError, "new directory"):
            self.verify()
        self.assertEqual(before, (self.output / "result.json").read_bytes())

    def test_symlink_and_hardlink_rejected(self):
        self.valid_control()
        alias = self.root / "alias"
        alias.symlink_to(self.binary)
        self.input["artifacts"]["nodeBinary"]["file"] = "alias"
        self.manifest.write_text(json.dumps(self.input))
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.verify()
        alias.unlink()
        os.link(self.binary, alias)
        with self.assertRaisesRegex(ValueError, "unaliased regular"):
            self.verify()

    def test_symlinked_parent_rejected(self):
        self.valid_control()
        alias = self.root / "parent-alias"
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            n.verify(alias / "manifest.json", self.output)

    def test_unknown_or_duplicate_json_rejected(self):
        self.valid_control()
        self.manifest.write_text('{"schema":"x","schema":"y"}')
        with self.assertRaisesRegex(ValueError, "duplicate JSON"):
            self.verify()
        with self.assertRaisesRegex(ValueError, "nonfinite"):
            n.strict_json(b'{"x":1e999}')

    def test_checkpoint_and_substitute_binding(self):
        self.valid_control()
        self.spec["codeSubstitutes"]["28191817"] = "0x00"
        self.refresh()
        with self.assertRaisesRegex(ValueError, "substitute bytes mismatch"):
            self.verify()


class ProcessTests(unittest.TestCase):
    def test_fixed_process_and_logs_recorded(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            class Process:
                pid = 12345
                def poll(self):
                    return 0
                def wait(self):
                    return 0
            def popen(argv, **kwargs):
                self.assertFalse(kwargs["shell"])
                self.assertTrue(kwargs["start_new_session"])
                self.assertEqual(kwargs["cwd"], root)
                self.assertEqual(kwargs["env"], {"LANG": "C"})
                kwargs["stdout"].write(b"synthetic stdout\n")
                return Process()
            with patch.object(n.subprocess, "Popen", side_effect=popen):
                receipt, data = n.run_fixed(["/synthetic/framenode", "--version"], root, "version", {"LANG": "C"}, 15)
            self.assertEqual(receipt["pid"], 12345)
            self.assertEqual(receipt["stdout"]["sha256"], n.sha(data))
            self.assertEqual(receipt["exitCode"], 0)
            started = n.strict_json((root / "version.started.json").read_bytes())
            self.assertEqual(started["pid"], 12345)
            self.assertIs(started["completionObserved"], False)
            self.assertEqual(receipt["startedReceipt"]["sha256"], n.sha((root / "version.started.json").read_bytes()))

    def test_interruption_retains_durable_launch_and_pid(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            class Process:
                pid = 12345
                calls = 0
                def poll(self):
                    self.calls += 1
                    if self.calls == 1:
                        raise KeyboardInterrupt()
                    return None
                def wait(self):
                    return -9
            with patch.object(n.subprocess, "Popen", return_value=Process()), \
                 patch.object(n.os, "killpg") as kill, self.assertRaises(KeyboardInterrupt):
                n.run_fixed(["/synthetic/framenode", "--version"], root, "version", {}, 15)
            self.assertEqual(kill.call_args.args[0], 12345)
            self.assertTrue((root / "version.launch.json").is_file())
            self.assertEqual(n.strict_json((root / "version.started.json").read_bytes())["pid"], 12345)
            self.assertFalse((root / "version.process.json").exists())

    def test_launch_failure_retains_intent_without_false_pid(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(n.subprocess, "Popen", side_effect=OSError("synthetic launch failure")), \
                 self.assertRaisesRegex(OSError, "synthetic launch failure"):
                n.run_fixed(["/synthetic/framenode", "--version"], root, "version", {}, 15)
            self.assertTrue((root / "version.launch.json").is_file())
            self.assertFalse((root / "version.started.json").exists())
            self.assertFalse((root / "version.process.json").exists())

    def test_nonzero_and_oversized_output_fail_and_retain_receipt(self):
        for exit_code, payload, expected in ((9, b"bad", "exit code 9"), (0, b"x" * (n.MAX_LOG + 1), "output bound")):
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                class Process:
                    pid = 12345
                    def poll(self):
                        return exit_code
                    def wait(self):
                        return exit_code
                def popen(argv, **kwargs):
                    kwargs["stdout"].write(payload)
                    return Process()
                with patch.object(n.subprocess, "Popen", side_effect=popen), self.assertRaisesRegex(ValueError, expected):
                    n.run_fixed(["/synthetic/framenode", "--version"], root, "version", {}, 15)
                self.assertTrue((root / "version.process.json").is_file())

    def test_timeout_kills_process_group(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            class Process:
                pid = 12345
                def poll(self):
                    return None
                def wait(self):
                    return -9
            with patch.object(n.subprocess, "Popen", return_value=Process()), \
                 patch.object(n.time, "monotonic", side_effect=[0, 20]), \
                 patch.object(n.os, "killpg") as kill, self.assertRaisesRegex(ValueError, "timeout"):
                n.run_fixed(["/synthetic/framenode", "--version"], root, "version", {}, 15)
            self.assertEqual(kill.call_args.args[0], 12345)
            self.assertTrue((root / "version.process.json").is_file())


if __name__ == "__main__":
    unittest.main()
