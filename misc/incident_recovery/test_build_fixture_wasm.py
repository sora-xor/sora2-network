import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_fixture_wasm as build
import fixture_artifacts
from test_release_package import policy, wasm


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

    def fixture_source(self):
        """A synthetic exported source, only for mocked-process build checks."""
        raw = policy()
        generated = self.root / "runtime/src/incident_recovery/generated.rs"
        generated.parent.mkdir(parents=True)
        generated.write_text(fixture_artifacts.final_source(raw))
        (self.root / "fixture-policy.scale").write_bytes(raw)
        (self.root / "Cargo.lock").write_text("version = 4\n")
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="nightly-2026-10-23"\n')
        (self.root / "NEVER-DEPLOY.txt").write_text(fixture_artifacts.FIXTURE)
        provenance = {"schema": "sora-nexus-final-fixture-source-v1", "fixtureOnly": True,
            "productionReady": False, "specVersion": 138, "transactionVersion": 134,
            "policy": build.digest(self.root / "fixture-policy.scale"),
            "policyIdentity": json.loads(json.dumps(build.package.policy_identity(raw))),
            "generatedPolicy": build.digest(generated),
            "sourceFiles": {name: build.digest(self.root / name) for name in ("Cargo.lock", "rust-toolchain.toml")}}
        (self.root / "FIXTURE-PROVENANCE.json").write_text(json.dumps(provenance))
        target = self.root / "target"
        (target / "debug/wbuild/framenode-runtime").mkdir(parents=True)
        return provenance, target

    def test_companion_requires_exact_final_fixture_policy_and_generated_source(self):
        provenance, _ = self.fixture_source()
        accepted = build.companion_policy(self.root, provenance)
        self.assertEqual(accepted["policyCommitment"], provenance["policyIdentity"]["commitment"])
        for change in ({"schema": "different"}, {"fixtureOnly": False}, {"productionReady": True},
                       {"specVersion": 135, "transactionVersion": 132}):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "exact public final-runtime"):
                build.companion_policy(self.root, {**provenance, **change})
        with self.assertRaisesRegex(ValueError, "policy differs"):
            build.companion_policy(self.root, {**provenance, "policyIdentity": {}})
        generated = self.root / "runtime/src/incident_recovery/generated.rs"
        generated.write_text(generated.read_text().replace("PUBLIC FIXTURE ONLY", "production candidate"))
        with self.assertRaisesRegex(ValueError, "exact public-fixture policy"):
            build.companion_policy(self.root, provenance)
        (self.root / "fixture-policy.scale").write_bytes(policy()[:-1] + b'\x08')
        with self.assertRaisesRegex(ValueError, "policy differs"):
            build.companion_policy(self.root, provenance)

    def test_nested_companion_features_allow_only_the_explicit_delta(self):
        _, target = self.fixture_source()
        project = target / "debug/wbuild/framenode-runtime"
        for features, companion, allowed in [
                (["build-wasm-binary"], False, True),
                (["try-runtime", "build-wasm-binary"], True, True),
                (["build-wasm-binary"], True, False),
                (["build-wasm-binary", "try-runtime"], False, False),
                (["build-wasm-binary", "try-runtime", "wip"], True, False),
                (["build-wasm-binary", "try-runtime", "try-runtime"], True, False)]:
            (project / "Cargo.toml").write_text('[dependencies.wasm-project]\npackage="framenode-runtime"\n'
                'default-features=false\nfeatures=' + json.dumps(features) + '\npath=' + json.dumps(str(self.root / "runtime")) + '\n')
            with self.subTest(features=features, companion=companion):
                if allowed:
                    self.assertEqual(build.validate_wasm_project(project, self.root, companion)["features"], features)
                else:
                    with self.assertRaisesRegex(ValueError, "Unexpected fixture Wasm features"):
                        build.validate_wasm_project(project, self.root, companion)

    def mock_successful_build(self, companion, mutation=None):
        _, target = self.fixture_source()
        project = target / "debug/wbuild/framenode-runtime"
        output = self.root / "evidence"
        def cargo(command, **kwargs):
            self.assertEqual(command[-2:], ["--features", ",".join(build.runtime_features(companion))])
            (project / "Cargo.toml").write_text('[dependencies.wasm-project]\npackage="framenode-runtime"\n'
                'default-features=false\nfeatures=' + json.dumps(build.runtime_features(companion))
                + '\npath=' + json.dumps(str(self.root / "runtime")) + '\n')
            for name in build.EXPORTS[1:]:
                (project / name).write_bytes(wasm(138, 134, b"synthetic companion" if companion else b"synthetic ordinary"))
            if mutation:
                mutation()
            return build.subprocess.CompletedProcess(command, 0)
        with patch.object(build.subprocess, "check_output", return_value="test-only rustc"), \
                patch.object(build.subprocess, "run", side_effect=cargo):
            return build.build_locked(self.root, output, None, target, companion)

    def test_companion_receipt_and_names_are_distinct_without_changing_source(self):
        report = self.mock_successful_build(True)
        self.assertEqual(report["schema"], "sora-nexus-fixture-wasm-build-v1")
        self.assertEqual(report["status"], "passed")
        self.assertEqual(report["artifactRole"], "tryRuntimeCompanion")
        self.assertTrue(report["tryRuntimeCompanion"])
        self.assertFalse(report["productionReady"])
        self.assertEqual(report["cargoFeatures"], ["build-wasm-binary", "try-runtime"])
        self.assertEqual(report["lockBefore"], report["lockAfter"])
        self.assertEqual(set(report["artifacts"]), {"runtime-138.try-runtime.compact.wasm",
            "runtime-138.try-runtime.compact.compressed.wasm"})
        self.assertFalse((self.root / "evidence/runtime-138.compact.compressed.wasm").exists())
        self.assertEqual(report["companionFixturePolicy"], build.companion_policy(
            self.root, build.source_provenance(self.root)[1]))

    def test_ordinary_build_keeps_existing_features_and_artifact_names(self):
        report = self.mock_successful_build(False)
        self.assertEqual(report["artifactRole"], "ordinaryRuntime")
        self.assertFalse(report["tryRuntimeCompanion"])
        self.assertEqual(report["cargoFeatures"], ["build-wasm-binary"])
        self.assertEqual(set(report["artifacts"]), {"runtime-138.compact.wasm", "runtime-138.compact.compressed.wasm"})
        self.assertNotIn("companionFixturePolicy", report)

    def test_companion_policy_change_during_mock_build_cannot_pass(self):
        def mutate():
            (self.root / "fixture-policy.scale").write_bytes(policy()[:-1] + b'\x08')
        with self.assertRaisesRegex(ValueError, "policy differs"):
            self.mock_successful_build(True, mutate)
        report = json.loads((self.root / "evidence/build.json").read_text())
        self.assertEqual(report["status"], "failed")
        self.assertNotIn("artifacts", report)

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
