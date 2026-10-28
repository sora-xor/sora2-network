import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_fixture_wasm as build


class BuildFixtureTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def test_environment_strips_skip_controls_without_recording_inherited_values(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="nightly-2026-10-23"\n')
        inherited = {"SKIP_WASM_BUILD": "1", "SKIP_FRAMENODE_RUNTIME_WASM_BUILD": "1", "DOCS_RS": "1", "RUSTFLAGS": "wrong", "PRIVATE_UNRELATED": "do-not-log", "DYLD_LIBRARY_PATH": "wrong"}
        with patch.dict(build.os.environ, inherited, clear=True):
            env, policy = build.build_environment(self.root, "0123456789abcdef0123456789abcdef")
        self.assertNotIn("SKIP_WASM_BUILD", env)
        self.assertNotIn("SKIP_FRAMENODE_RUNTIME_WASM_BUILD", env)
        self.assertNotIn("DOCS_RS", env)
        self.assertNotIn("DYLD_LIBRARY_PATH", env)
        self.assertNotIn("do-not-log", json.dumps(policy))
        self.assertEqual(env["CARGO_TARGET_DIR"], str(self.root.resolve() / "target"))
        self.assertEqual(env["WASM_BUILD_WORKSPACE_HINT"], str(self.root.resolve()))
        self.assertEqual(env["WASM_BUILD_TOOLCHAIN"], "nightly-2026-10-23")

    def test_runner_requires_explicit_fixture_provenance(self):
        (self.root / "FIXTURE-PROVENANCE.json").write_text(json.dumps({"fixtureOnly": True, "specVersion": 138, "transactionVersion": 134}))
        with self.assertRaisesRegex(ValueError, "NEVER-DEPLOY"):
            build.source_provenance(self.root)
        (self.root / "NEVER-DEPLOY.txt").write_text("NEVER DEPLOY")
        self.assertEqual(build.source_provenance(self.root)[1]["specVersion"], 138)
        (self.root / "FIXTURE-PROVENANCE.json").write_text(json.dumps({"fixtureOnly": False, "specVersion": 138, "transactionVersion": 134}))
        with self.assertRaisesRegex(ValueError, "not a fixture"):
            build.source_provenance(self.root)

    def test_environment_uses_one_explicit_nested_offline_argument(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="nightly-2026-10-23"\n')
        for inherited_offline in ("true", "false", "private-invalid-value"):
            with self.subTest(inherited_offline=inherited_offline):
                inherited = {"CARGO_NET_OFFLINE": inherited_offline,
                             "WASM_BUILD_CARGO_ARGS": "--offline --offline --locked"}
                with patch.dict(build.os.environ, inherited, clear=True):
                    env, policy = build.build_environment(self.root, "0123456789abcdef0123456789abcdef")
                    self.assertEqual(dict(build.os.environ), inherited)
                self.assertNotIn("CARGO_NET_OFFLINE", env)
                self.assertNotIn("CARGO_NET_OFFLINE", policy["controlled"])
                self.assertIn("CARGO_NET_OFFLINE", policy["removedKeys"])
                self.assertIn("WASM_BUILD_CARGO_ARGS", policy["removedKeys"])
                self.assertEqual(env["WASM_BUILD_CARGO_ARGS"].split(), ["--offline"])
                self.assertEqual(policy["controlled"]["WASM_BUILD_CARGO_ARGS"], "--offline")
                self.assertNotIn("private-invalid-value", json.dumps(policy))

    def test_runner_keeps_parent_and_nested_cargo_explicitly_offline(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="nightly-2026-10-23"\n')
        (self.root / "Cargo.lock").write_text("version = 4\n")
        (self.root / "NEVER-DEPLOY.txt").write_text("NEVER DEPLOY")
        (self.root / "FIXTURE-PROVENANCE.json").write_text(json.dumps({
            "fixtureOnly": True, "specVersion": 138, "transactionVersion": 134,
        }))
        target = self.root / "target"
        (target / "debug/wbuild/framenode-runtime").mkdir(parents=True)
        output = self.root / "evidence"
        # Exercise the real runner/environment path without executing rustc or
        # Cargo. A deliberate nonzero exit stops before any artifact is accepted.
        with patch.dict(build.os.environ, {"CARGO_NET_OFFLINE": "true"}, clear=True), \
                patch.object(build, "fingerprint", return_value={}), \
                patch.object(build.subprocess, "check_output", return_value="test-only rustc"), \
                patch.object(build.subprocess, "run", return_value=build.subprocess.CompletedProcess([], 19)) as cargo:
            with self.assertRaisesRegex(ValueError, "Cargo fixture build failed with exit 19"):
                build.build_locked(self.root, output, None, target)
            self.assertEqual(build.os.environ["CARGO_NET_OFFLINE"], "true")
        command = cargo.call_args.args[0]
        environment = cargo.call_args.kwargs["env"]
        self.assertEqual(command.count("--offline"), 1)
        self.assertEqual(command.count("--locked"), 1)
        self.assertNotIn("CARGO_NET_OFFLINE", environment)
        self.assertEqual(environment["WASM_BUILD_CARGO_ARGS"].split(), ["--offline"])
        report = json.loads((output / "build.json").read_text())
        self.assertEqual(report["command"], command)
        self.assertEqual(report["exitCode"], 19)
        self.assertEqual(report["status"], "failed")
        self.assertIn("CARGO_NET_OFFLINE", report["environmentPolicy"]["removedKeys"])

    def test_shared_target_retains_source_hint_and_excludes_concurrent_builds(self):
        source, target = self.root / "hold", self.root / "warm-target"
        source.mkdir()
        target.mkdir()
        (source / "rust-toolchain.toml").write_text('[toolchain]\nchannel="nightly-2025-05-08"\n')
        env, policy = build.build_environment(source, "0123456789abcdef0123456789abcdef", target)
        self.assertEqual(env["CARGO_TARGET_DIR"], str(target.resolve()))
        self.assertEqual(policy["controlled"]["CARGO_TARGET_DIR"], str(target.resolve()))
        self.assertEqual(env["WASM_BUILD_WORKSPACE_HINT"], str(source.resolve()))
        with build.target_lock(target), self.assertRaisesRegex(ValueError, "target lane"):
            with build.target_lock(target):
                self.fail("Concurrent target access was admitted")

    def test_quarantine_retains_compiler_cache_and_removes_all_stale_exports(self):
        project, output = self.root / "project", self.root / "evidence"
        project.mkdir()
        output.mkdir()
        for name in build.EXPORTS:
            (project / name).write_text("stale")
        (project / "framenode_runtime.wasm").write_text("warm bloaty code")
        prior = build.quarantine(project, output)
        self.assertEqual(len(prior), 3)
        self.assertTrue(all(not (project / name).exists() for name in build.EXPORTS))
        self.assertEqual((project / "framenode_runtime.wasm").read_text(), "warm bloaty code")

    def test_wasm_project_cannot_adopt_same_version_from_another_source(self):
        source, previous, project = self.root / "current", self.root / "old", self.root / "wbuild"
        for path in (source / "runtime", previous / "runtime", project):
            path.mkdir(parents=True)
        def write_manifest(runtime):
            (project / "Cargo.toml").write_text('[dependencies.wasm-project]\npackage="framenode-runtime"\ndefault-features=false\nfeatures=["build-wasm-binary"]\npath=' + json.dumps(str(runtime)) + '\n')
        write_manifest(previous / "runtime")
        with self.assertRaisesRegex(ValueError, "different fixture source"):
            build.validate_wasm_project(project, source)
        write_manifest(source / "runtime")
        self.assertEqual(build.validate_wasm_project(project, source)["package"], "framenode-runtime")

    def test_legacy_lock_accepts_only_reviewed_direct_edges_and_no_version_drift(self):
        old = '''version = 4
[[package]]
name = "framenode-runtime"
version = "4.8.12"
dependencies = ["base"]
[[package]]
name = "base"
version = "1.0.0"
'''
        (self.root / "Cargo.lock").write_text(old)
        reference = self.root / "resolved.lock"
        reference.write_text(old.replace('["base"]', '["base", "sp-trie", "sp-externalities"]'))
        evidence = build.resolve_legacy_lock(self.root, reference, {"specVersion": 135})
        self.assertFalse(evidence["packageIdentityDrift"])
        reference.write_text(reference.read_text().replace('version = "1.0.0"', 'version = "1.0.1"'))
        with self.assertRaisesRegex(ValueError, "identities"):
            build.resolve_legacy_lock(self.root, reference, {"specVersion": 135})


if __name__ == "__main__":
    unittest.main()
