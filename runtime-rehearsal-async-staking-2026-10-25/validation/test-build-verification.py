#!/usr/bin/env python3
"""Test production Wasm export freshness without running Cargo or touching its cache."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


VALIDATION = Path(__file__).resolve().parent
RUNNER = VALIDATION / 'run-build-verification.py'
if str(VALIDATION) not in sys.path:
    sys.path.insert(0, str(VALIDATION))


def load_runner():
    spec = importlib.util.spec_from_file_location('build_verification_under_test', RUNNER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load_runner()
MANIFEST = '''[dependencies.wasm-project]
package = "framenode-runtime"
path = "/test/runtime"
default-features = false
features = ["build-wasm-binary"]
'''


class BuildVerificationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='build-export-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.project = self.root / 'target/debug/wbuild/framenode-runtime'
        self.project.mkdir(parents=True)
        self.archive = self.root / 'evidence/previous-exports'

    def write_exports(self, marker=b'fresh'):
        (self.project / 'Cargo.toml').write_text(MANIFEST)
        (self.project / 'framenode_runtime.compact.wasm').write_bytes(b'compact:' + marker)
        (self.project / 'framenode_runtime.compact.compressed.wasm').write_bytes(
            b'compressed:' + marker)
        return {name: (self.project / name).read_bytes() for name in runner.EXPORTS}

    def test_import_does_not_run_build_or_write_evidence(self):
        with mock.patch('subprocess.run', side_effect=AssertionError('unexpected process')), \
                mock.patch('subprocess.check_output', side_effect=AssertionError('unexpected process')), \
                mock.patch.object(Path, 'mkdir', side_effect=AssertionError('unexpected mkdir')), \
                mock.patch.object(Path, 'write_text', side_effect=AssertionError('unexpected write')):
            imported = load_runner()
        self.assertTrue(callable(imported.main))

    def test_quarantine_moves_only_exports_and_keeps_compiler_cache(self):
        previous = self.write_exports(b'old')
        sentinels = {
            self.project / 'framenode_runtime.wasm': b'keep bloaty Wasm',
            self.project / 'target/wasm32v1-none/release/deps/compiler-sentinel': b'keep compiler cache',
        }
        for path, data in sentinels.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        result = runner.quarantine_wasm_exports(self.project, self.archive)
        self.assertEqual(result['absentBeforeBuild'], list(runner.EXPORTS))
        self.assertEqual([row['name'] for row in result['previousExports']], list(runner.EXPORTS))
        for row in result['previousExports']:
            name = row['name']
            self.assertFalse((self.project / name).exists())
            self.assertEqual((self.archive / name).read_bytes(), previous[name])
            self.assertEqual(row['sha256'], hashlib.sha256(previous[name]).hexdigest())
            self.assertEqual(row['bytes'], len(previous[name]))
            self.assertEqual(Path(row['archivePath']), self.archive / name)
        for path, data in sentinels.items():
            self.assertEqual(path.read_bytes(), data)

    def test_existing_archive_cannot_be_reused_or_overwritten(self):
        self.write_exports(b'old')
        runner.quarantine_wasm_exports(self.project, self.archive)
        recreated = self.write_exports(b'new')
        with self.assertRaisesRegex(RuntimeError, 'archive already exists'):
            runner.quarantine_wasm_exports(self.project, self.archive)
        for name, data in recreated.items():
            self.assertEqual((self.project / name).read_bytes(), data)
        self.assertEqual((self.archive / runner.EXPORTS[1]).read_bytes(), b'compact:old')

    def test_quarantine_rejects_export_symlink_without_reading_target(self):
        target = self.root / 'external-export'
        target.write_bytes(b'not a generated export')
        (self.project / runner.EXPORTS[0]).symlink_to(target)
        with self.assertRaisesRegex(RuntimeError, 'Unexpected export symlink'):
            runner.quarantine_wasm_exports(self.project, self.archive)
        self.assertEqual(target.read_bytes(), b'not a generated export')
        self.assertTrue((self.project / runner.EXPORTS[0]).is_symlink())

    def test_no_op_build_cannot_accept_quarantined_stale_exports(self):
        previous = self.write_exports(b'stale')
        runner.quarantine_wasm_exports(self.project, self.archive)
        # A skipped Cargo build exits successfully but leaves these exports absent.
        with self.assertRaisesRegex(RuntimeError, 'did not recreate Cargo.toml'):
            runner.verify_wasm_exports(self.project)
        for name, data in previous.items():
            self.assertEqual((self.archive / name).read_bytes(), data)

    def test_compact_wasm_alone_does_not_replace_compressed_export(self):
        self.write_exports()
        (self.project / runner.EXPORTS[2]).unlink()
        with self.assertRaisesRegex(RuntimeError, 'did not recreate framenode_runtime.compact.compressed.wasm'):
            runner.verify_wasm_exports(self.project)

    def test_wasm_without_generated_manifest_is_rejected(self):
        self.write_exports()
        (self.project / runner.EXPORTS[0]).unlink()
        with self.assertRaisesRegex(RuntimeError, 'did not recreate Cargo.toml'):
            runner.verify_wasm_exports(self.project)

    def test_nonproduction_manifest_is_rejected(self):
        self.write_exports()
        for manifest, error in (
            (MANIFEST.replace('default-features = false', 'default-features = true'), 'default features'),
            (MANIFEST.replace('default-features = false\n', ''), 'default features'),
            (MANIFEST.replace('["build-wasm-binary"]', '["build-wasm-binary", "try-runtime"]'), 'Nonproduction'),
            (MANIFEST.replace('["build-wasm-binary"]', '[]'), 'Nonproduction'),
        ):
            with self.subTest(manifest=manifest):
                (self.project / 'Cargo.toml').write_text(manifest)
                with self.assertRaisesRegex(RuntimeError, error):
                    runner.verify_wasm_exports(self.project)

    def test_symlinked_recreated_export_is_rejected(self):
        for name in runner.EXPORTS:
            with self.subTest(name=name):
                self.write_exports()
                path = self.project / name
                target = self.root / ('target-' + name)
                path.rename(target)
                path.symlink_to(target)
                with self.assertRaisesRegex(RuntimeError, 'did not recreate'):
                    runner.verify_wasm_exports(self.project)
                path.unlink()

    def test_empty_recreated_export_is_rejected(self):
        for name in runner.EXPORTS:
            with self.subTest(name=name):
                self.write_exports()
                (self.project / name).write_bytes(b'')
                with self.assertRaisesRegex(RuntimeError, 'did not recreate'):
                    runner.verify_wasm_exports(self.project)

    def test_three_recreated_production_exports_are_bound_to_exact_bytes(self):
        old = self.write_exports(b'old')
        runner.quarantine_wasm_exports(self.project, self.archive)
        fresh = self.write_exports(b'new')
        dependency, records = runner.verify_wasm_exports(self.project)
        self.assertIs(dependency['default-features'], False)
        self.assertEqual(dependency['features'], ['build-wasm-binary'])
        self.assertEqual([row['name'] for row in records], list(runner.EXPORTS))
        for row in records:
            name = row['name']
            self.assertEqual(row['sha256'], hashlib.sha256(fresh[name]).hexdigest())
            self.assertEqual(row['bytes'], len(fresh[name]))
            self.assertEqual((self.archive / name).read_bytes(), old[name])

    def test_optimized_python_fails_with_report_before_any_build_subprocess(self):
        (self.root / 'rust-toolchain.toml').write_text(
            '[toolchain]\nchannel = "nightly-2026-10-23"\n')
        validation = self.root / 'validation'
        validation.mkdir()
        code = r'''
import importlib.util
from pathlib import Path
import sys
sys.path.insert(0, sys.argv[1])
spec = importlib.util.spec_from_file_location('optimized_build_runner', Path(sys.argv[1]) / 'run-build-verification.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
runner.ROOT = Path(sys.argv[2])
runner.V = runner.ROOT / 'validation'
runner.L = runner.V / 'final-build-logs'
runner.F = runner.V / 'source-clippy-start.json'
runner.REPORT = runner.V / 'final-build-verification.json'
def unexpected_process(*args, **kwargs):
    (runner.V / 'unexpected-subprocess').write_text('called')
    raise RuntimeError('Build subprocess must not run under optimized Python')
runner.subprocess.run = unexpected_process
runner.subprocess.check_output = unexpected_process
raise SystemExit(runner.main([]))
'''
        result = subprocess.run([sys.executable, '-O', '-c', code, str(VALIDATION), str(self.root)],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        report = json.loads((validation / 'final-build-verification.json').read_text())
        self.assertEqual(report['status'], 'failed')
        self.assertIn('do not use -O', report['error'])
        self.assertEqual(report['steps'], [])
        self.assertFalse((validation / 'unexpected-subprocess').exists())
        self.assertFalse((validation / 'candidate-build-evidence.json').exists())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--include-environment', action='store_true',
                        help='Also run the environment policy tests through explicit imports')
    args = parser.parse_args()
    loader = unittest.defaultTestLoader
    suite = unittest.TestSuite([loader.loadTestsFromTestCase(BuildVerificationTests)])
    if args.include_environment:
        spec = importlib.util.spec_from_file_location(
            'build_environment_tests', VALIDATION / 'test-build-environment.py')
        environment_tests = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(environment_tests)
        suite.addTests(loader.loadTestsFromTestCase(environment_tests.BuildEnvironmentTests))
    if suite.countTestCases() == 0:
        print('Build verification refused an empty test suite', file=sys.stderr)
        return 2
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    raise SystemExit(main())
