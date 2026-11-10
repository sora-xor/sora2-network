"""Offline recorder-producer regressions; no compiler or native process."""
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest
from types import SimpleNamespace

import production_native as native
import test_production_native as fixtures


class OriginalRecorderProducerTests(unittest.TestCase):
    def setUp(self):
        self.fixture_helper = fixtures.ProductionNativeTests()
        self.fixture_helper.setUp()
        self.addCleanup(self.fixture_helper.doCleanups)

    def fixture(self, version):
        return self.fixture_helper.fixture(version)

    def refresh_report(self, args, change):
        return self.fixture_helper.refresh_report(args, change)
    def test_original_preparer_embeds_required_amendments_and_installs_actual_lock(self):
        args = self.fixture(134)
        _, _, original, _ = native.reviewed_assets()
        request = SimpleNamespace(source=args.source, build_receipt=args.wasm_build,
            build_receipt_sha256=args.wasm_build_sha256, wasm=args.wasm,
            wasm_sha256=args.wasm_sha256, policy=args.policy,
            policy_sha256=args.policy_sha256, destination=args.destination)
        result = original.prepare(request)
        binding = native.parse(native.read(args.destination / 'RECORDER-ENGINE-BINDING.json'))
        generated = native.read(args.destination / 'utils/native-operation-recorder/src/binding.rs')
        marker = json.dumps(binding, sort_keys=True, separators=(',', ':')).encode()
        self.assertIn(marker, generated)
        self.assertEqual(binding['runtimeFeatureProfile']['businessRuntimeCfgFeatures'], [])
        self.assertEqual(binding['hostLockAmendment']['originalSha256'], native.digest(args.source / 'Cargo.lock')['sha256'])
        self.assertEqual(binding['hostLockAmendment']['resolvedSha256'], native.digest(args.destination / 'Cargo.lock')['sha256'])
        self.assertEqual(binding['sdkCompilationRepair']['assetManifestSha256'], native.digest(native.ASSETS / 'UPSTREAM.json')['sha256'])
        self.assertEqual(native.read(args.destination / 'RECORDER-ORIGINAL-Cargo.lock'), native.read(args.source / 'Cargo.lock'))
        self.assertEqual(native.read(args.destination / 'Cargo.lock'), native.read(native.ASSETS / 'repaired/lock/amended.lock'))
        self.assertEqual(set(result['amendedHostOnlySources']), {'Cargo.toml', 'Cargo.lock', 'runtime/src/lib.rs'})
        self.assertFalse(result['nativeBuildExecuted'])
        self.assertFalse(binding['nativeCompilerProvenanceVerified'])
        inventory = native.parse(native.read(args.destination / 'RECORDER-SOURCE-FILES.json'))
        self.assertEqual(inventory['Cargo.lock']['sha256'], native.digest(args.destination / 'Cargo.lock')['sha256'])

    def test_original_preparer_rejects_unsupported_actual_wasm_feature_profile(self):
        args = self.fixture(134)
        self.refresh_report(args, lambda report: report['wasmProjectDependency'].update(features=['different']))
        _, _, original, _ = native.reviewed_assets()
        with self.assertRaisesRegex(ValueError, 'feature|profile'):
            original.validate(args.source, args.wasm_build, args.wasm_build_sha256,
                args.wasm, args.wasm_sha256, args.policy, args.policy_sha256)


class AmendmentDerivationTests(unittest.TestCase):
    def setUp(self):
        self.helper = native.load_module('test_native_binding_amendments_helper', native.ASSETS / 'binding_amendments.py')

    def test_existing_contradictory_amendment_rejected(self):
        test = fixtures.ProductionNativeTests();test.setUp();self.addCleanup(test.doCleanups)
        args = test.fixture(134)
        _, lock, original, repaired = native.reviewed_assets()
        _, binding = original.validate(args.source, args.wasm_build, args.wasm_build_sha256,
            args.wasm, args.wasm_sha256, args.policy, args.policy_sha256)
        report = native.parse(native.read(args.wasm_build))
        again, host = self.helper.derive(binding, report, args.source, native.ASSETS, repaired.feature_profile, lock.check)
        self.assertEqual(again, binding)
        self.assertEqual(host, native.read(native.ASSETS / 'repaired/lock/amended.lock'))
        for key in ('runtimeFeatureProfile', 'hostLockAmendment', 'sdkCompilationRepair'):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'already bound'):
                self.helper.derive({**binding, key: {}}, report, args.source, native.ASSETS, repaired.feature_profile, lock.check)

    def test_direct_cli_module_load_disables_bytecode_before_loading_helpers(self):
        # The advertised direct Python CLI need not pass -B. This child only
        # loads the producer and its pure helper, without preparing or running
        # a runtime, Cargo or a database.
        script = (
            "import importlib.util,sys; from pathlib import Path; "
            "p=Path(sys.argv[1]); ns={'__name__':'source_only_probe','__file__':str(p)}; "
            "exec(compile(p.read_bytes(),str(p),'exec'),ns); "
            "assert sys.dont_write_bytecode; "
            "h=p.parent.parent/'binding_amendments.py'; "
            "s=importlib.util.spec_from_file_location('probe_binding_helper',h); "
            "m=importlib.util.module_from_spec(s);s.loader.exec_module(m); "
            "m.reviewed_manifest(p.parent.parent)"
        )
        result = subprocess.run([sys.executable, '-c', script,
            str(native.ASSETS / 'original/prepare.py')], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((native.ASSETS / '__pycache__').exists())

    def test_manifest_cannot_claim_unknown_or_changed_template(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / 'UPSTREAM.json').write_text(json.dumps({'schema': 'sora-nexus-production-native-reviewed-assets-v1', 'files': {}}))
            (root / 'unexpected.rs').write_bytes(b'changed recorder')
            with self.assertRaisesRegex(ValueError, 'unbound'):
                self.helper.reviewed_manifest(root)
            (root / 'UPSTREAM.json').write_text(json.dumps({'schema': 'sora-nexus-production-native-reviewed-assets-v1', 'files': {'unexpected.rs': {'sha256': '0' * 64}}}))
            with self.assertRaisesRegex(ValueError, 'digest'):
                self.helper.reviewed_manifest(root)


if __name__ == '__main__':
    unittest.main()
