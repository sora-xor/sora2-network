#!/usr/bin/env python3
"""Regression checks for the production-build environment policy; no Cargo runs."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import uuid

import build_environment


HELPER = Path(build_environment.__file__).resolve()
SCRIPT = Path(__file__).resolve()
TOOLCHAIN = "nightly-2026-10-23"
TOKEN = "test-build-environment-00000001"
# Faithful to substrate-wasm-builder's check_skip_build(): presence is enough,
# including an empty string or the text 'false'. A skipped build leaves an old
# artifact untouched. This executes in a child process with the prepared env.
SDK_BUILD_STAND_IN = r'''
import json, os
from pathlib import Path
artifact = Path(os.environ["TEST_ARTIFACT"])
skip = any(key in os.environ for key in (
    "SKIP_WASM_BUILD", "SKIP_FRAMENODE_RUNTIME_WASM_BUILD", "DOCS_RS"))
if not skip:
    artifact.write_text("fresh:" + os.environ["FORCE_WASM_BUILD"])
print(json.dumps({"skipped": skip, "artifact": artifact.read_text()}))
'''


class BuildEnvironmentTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="build-environment-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / "rust-toolchain.toml").write_text(
            f'[toolchain]\nchannel = "{TOOLCHAIN}"\ntargets = ["wasm32v1-none"]\n')

    def prepare(self, environment, token=TOKEN):
        return build_environment.prepare_build_environment(environment, self.root, run_token=token)

    def test_sdk_skip_presence_cannot_reuse_warm_artifact(self):
        artifact = self.root / "candidate.wasm"
        for key in ("SKIP_WASM_BUILD", "SKIP_FRAMENODE_RUNTIME_WASM_BUILD", "DOCS_RS"):
            for value in ("", "false", "0", "1"):
                with self.subTest(key=key, value=value):
                    artifact.write_text("stale-artifact")
                    inherited = {"TEST_ARTIFACT": str(artifact), key: value}
                    before = subprocess.run([sys.executable, "-c", SDK_BUILD_STAND_IN],
                                            env=inherited, capture_output=True, text=True, check=True)
                    self.assertEqual(json.loads(before.stdout),
                                     {"skipped": True, "artifact": "stale-artifact"})
                    environment, metadata = self.prepare(inherited)
                    after = subprocess.run([sys.executable, "-c", SDK_BUILD_STAND_IN],
                                           env=environment, capture_output=True, text=True, check=True)
                    self.assertEqual(json.loads(after.stdout),
                                     {"skipped": False, "artifact": "fresh:" + TOKEN})
                    self.assertIn(key, metadata["removedKeys"])

    def test_every_project_skip_flag_is_removed_independently_of_value(self):
        keys = ["SKIP_ANOTHER_PROJECT_WASM_BUILD", "SKIP__WASM_BUILD", "SKIP_WASM_BUILD"]
        inherited = dict(zip(keys, ("", "false", "0")))
        environment, metadata = self.prepare(inherited)
        self.assertEqual(metadata["removedKeys"], sorted(keys))
        self.assertTrue(all(key not in environment for key in keys))

    def test_nested_wasm_compiler_target_profile_and_wrapper_overrides_are_removed(self):
        keys = [
            "WASM_BUILD_RUSTFLAGS", "WASM_BUILD_CARGO_ARGS", "WASM_BUILD_TOOLCHAIN",
            "WASM_BUILD_STD", "WASM_BUILD_WORKSPACE_HINT", "WASM_TARGET_DIRECTORY",
            "WASM_BUILD_TYPE", "WASM_BUILD_NO_COLOR", "WASM_BUILD_FUTURE_OVERRIDE",
            "SUBSTRATE_RUNTIME_TARGET", "FORCE_WASM_BUILD", "CARGO_TARGET_DIR",
            "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS",
            "RUSTUP_TOOLCHAIN", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
            "RUSTC_BOOTSTRAP", "RUSTC_FORCE_INCREMENTAL", "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTC",
            "CARGO_BUILD_RUSTDOC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTDOCFLAGS",
            "CARGO_TARGET_WASM32V1_NONE_RUSTFLAGS", "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS",
            "CARGO_TARGET_WASM32V1_NONE_LINKER", "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER",
            "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", "CARGO_PROFILE_DEV_OPT_LEVEL",
            "LIBRARY_PATH", "LIBCLANG_PATH", "LLVM_CONFIG_PATH",
            "PYTHONOPTIMIZE", "PYTHONPATH", "PYTHONHOME",
        ]
        inherited = {key: "untrusted-override" for key in keys}
        environment, metadata = self.prepare(inherited)
        self.assertEqual(metadata["removedKeys"], sorted(keys))
        self.assertNotIn("untrusted-override", environment.values())
        self.assertEqual(environment, metadata["controlled"])
        self.assertEqual(environment["RUSTUP_TOOLCHAIN"], TOOLCHAIN)
        self.assertEqual(environment["WASM_BUILD_TOOLCHAIN"], TOOLCHAIN)
        self.assertEqual(environment["CARGO_TARGET_DIR"], str(self.root / "target"))
        self.assertEqual(environment["WASM_BUILD_TYPE"], "release")
        self.assertEqual(environment["WASM_BUILD_RUSTFLAGS"], "-C link-arg=--import-undefined")
        self.assertEqual(environment["SUBSTRATE_RUNTIME_TARGET"], "wasm")
        self.assertEqual(environment["WASM_BUILD_STD"], "0")
        self.assertEqual(environment["FORCE_WASM_BUILD"], TOKEN)
        self.assertEqual(environment["LIBCLANG_PATH"], "/opt/homebrew/opt/llvm@22/lib")

    def test_input_is_not_mutated_and_evidence_cannot_leak_inherited_values(self):
        inherited = {"RUSTFLAGS": "secret-in-overridden-variable", "SERVICE_TOKEN": "secret-token",
                     "PATH": "/bin", "CARGO_BUILD_JOBS": "6", "CARGO_HOME": "/warm/cargo",
                     "RUSTUP_HOME": "/warm/rustup"}
        original = inherited.copy()
        process_before = os.environ.copy()
        environment, metadata = self.prepare(inherited)
        self.assertEqual(inherited, original)
        self.assertEqual(dict(os.environ), process_before)
        self.assertIsNot(environment, inherited)
        self.assertEqual(environment["SERVICE_TOKEN"], "secret-token")
        self.assertEqual(environment["CARGO_BUILD_JOBS"], "6")
        self.assertEqual(environment["CARGO_HOME"], "/warm/cargo")
        self.assertEqual(environment["RUSTUP_HOME"], "/warm/rustup")
        encoded = json.dumps(metadata)
        self.assertNotIn("secret", encoded)
        self.assertNotIn("SERVICE_TOKEN", encoded)
        self.assertEqual(metadata["removedKeys"], ["RUSTFLAGS"])
        self.assertFalse((self.root / "target").exists())

    def test_toolchain_is_bound_to_manifest_bytes(self):
        _, metadata = self.prepare({})
        self.assertEqual(metadata["toolchainFile"], "rust-toolchain.toml")
        self.assertEqual(metadata["toolchainFileSha256"],
                         hashlib.sha256((self.root / "rust-toolchain.toml").read_bytes()).hexdigest())
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.90.0"\n')
        environment, exact = self.prepare({})
        self.assertEqual(environment["RUSTUP_TOOLCHAIN"], "1.90.0")
        self.assertEqual(environment["WASM_BUILD_TOOLCHAIN"], "1.90.0")
        self.assertNotEqual(metadata["toolchainFileSha256"], exact["toolchainFileSha256"])

    def test_floating_or_path_toolchain_is_rejected(self):
        for channel in ("nightly", "stable", "beta", "/tmp/unbound-toolchain"):
            with self.subTest(channel=channel):
                (self.root / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel = "{channel}"\n')
                with self.assertRaisesRegex(ValueError, "must pin"):
                    self.prepare({})

    def test_fresh_run_tokens_replace_inherited_force_value(self):
        first = uuid.uuid4().hex
        second = uuid.uuid4().hex
        self.assertNotEqual(first, second)
        one, _ = self.prepare({"FORCE_WASM_BUILD": "inherited"}, first)
        two, _ = self.prepare({"FORCE_WASM_BUILD": "inherited"}, second)
        self.assertEqual(one["FORCE_WASM_BUILD"], first)
        self.assertEqual(two["FORCE_WASM_BUILD"], second)
        for token in ("", "short", "line\nbreak-identifier", "x" * 129):
            with self.subTest(token=token), self.assertRaisesRegex(ValueError, "run_token"):
                self.prepare({}, token)

    def test_same_inputs_are_deterministic(self):
        inherited = {"PATH": "/bin", "SKIP_FRAMENODE_RUNTIME_WASM_BUILD": "false"}
        self.assertEqual(self.prepare(inherited), self.prepare(inherited))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(BuildEnvironmentTests))
    report = {
        "status": "passed" if result.wasSuccessful() else "failed",
        "testsRun": result.testsRun, "failures": len(result.failures), "errors": len(result.errors),
        "helperSha256": hashlib.sha256(HELPER.read_bytes()).hexdigest(),
        "testScriptSha256": hashlib.sha256(SCRIPT.read_bytes()).hexdigest(),
        "details": stream.getvalue(),
    }
    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded)
    print(encoded, end="")
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
