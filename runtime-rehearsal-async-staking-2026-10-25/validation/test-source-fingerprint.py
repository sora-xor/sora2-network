#!/usr/bin/env python3
"""Offline regression fixtures; temporary repositories never touch the user index."""
import copy
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).with_name('source-fingerprint.py')
spec = importlib.util.spec_from_file_location('source_fingerprint', SCRIPT)
fingerprint = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fingerprint)


class FingerprintTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='sora2-source-fingerprint-test-')
        self.root = Path(self.temporary.name).resolve()
        self.evidence = self.root / 'rehearsal'
        self.write('Cargo.toml', '[workspace]\nmembers = ["runtime", "future-crate"]\n')
        self.write('runtime/Cargo.toml', '[package]\nname="runtime"\nversion="0.1.0"\n')
        self.write('runtime/src/lib.rs', 'pub fn value() -> u8 { 1 }\n')
        self.write('future-crate/Cargo.toml', '[package]\nname="future"\nversion="0.1.0"\n')
        self.write('future-crate/src/lib.rs', 'pub const FUTURE: u8 = 7;\n')
        self.write('vendor/crate/Cargo.toml', '[package]\nname="vendor"\nversion="0.1.0"\n')
        self.write('vendor/crate/README.md', 'Documentation.\n')
        self.write('.gitignore', 'vendor/ignored/\n.cargo/\n**/target/\n')
        self.git('init', '-q')
        self.git('add', '.')
        self.git('-c', 'user.name=Source Fixture', '-c', 'user.email=fixture.invalid@example.invalid', 'commit', '-qm', 'fixture baseline')
        self.write('rehearsal/validation/check.cjs', "console.log('fixture');\n")

    def tearDown(self):
        self.temporary.cleanup()

    def write(self, relative, content):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content if isinstance(content, bytes) else content.encode())
        return path

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.root), *args], stderr=subprocess.DEVNULL)

    def capture(self):
        return fingerprint.collect(self.root, self.evidence, [])

    def test_stable_capture_preserves_user_index(self):
        index_before = (self.root / '.git/index').read_bytes()
        one, two = self.capture(), self.capture()
        self.assertEqual(one['source']['sha256'], two['source']['sha256'])
        self.assertEqual(one['git']['overlaySha256'], two['git']['overlaySha256'])
        self.assertEqual(index_before, (self.root / '.git/index').read_bytes())

    def test_new_workspace_crate_and_ignored_vendor_binary_are_inputs(self):
        before = self.capture()
        self.write('vendor/ignored/Cargo.lock', 'version = 4\n')
        self.write('vendor/ignored/vector.dat', bytes(range(256)))
        self.write('.cargo/config.toml', '[build]\nincremental = true\n')
        after = self.capture()
        paths = {row['path'] for row in after['source']['files']}
        self.assertTrue({'future-crate/src/lib.rs', 'vendor/ignored/Cargo.lock', 'vendor/ignored/vector.dat', '.cargo/config.toml'} <= paths)
        self.assertNotEqual(before['source']['sha256'], after['source']['sha256'])

    def test_staged_and_unstaged_deletions_are_retained(self):
        (self.root / 'runtime/src/lib.rs').unlink()
        self.git('rm', '-q', 'future-crate/src/lib.rs')
        after = self.capture()
        deleted = {row['path'] for row in after['source']['files'] if row['kind'] == 'deleted'}
        self.assertEqual(deleted, {'runtime/src/lib.rs', 'future-crate/src/lib.rs'})
        self.assertEqual({row['path'] for row in after['git']['overlay'] if row['state'] == 'deleted'}, deleted)

    def test_generated_caches_and_evidence_do_not_change_source(self):
        before = self.capture()
        self.write('runtime/target/generated.rs', 'untrusted generated output\n')
        self.write('vendor/crate/node_modules/pkg/index.js', 'generated dependency output\n')
        self.write('rehearsal/validation/candidate.wasm', b'generated candidate')
        self.write('rehearsal/validation/report.json', '{"status":"running"}\n')
        after = self.capture()
        self.assertEqual(before['source']['sha256'], after['source']['sha256'])
        self.assertEqual(before['tooling']['sha256'], after['tooling']['sha256'])

    def test_docs_and_tooling_are_separate_from_source(self):
        before = self.capture()
        self.write('vendor/crate/README.md', 'Updated explanation.\n')
        self.write('rehearsal/validation/check.cjs', "console.log('changed fixture');\n")
        after = self.capture()
        self.assertEqual(before['source']['sha256'], after['source']['sha256'])
        self.assertNotEqual(before['supporting']['sha256'], after['supporting']['sha256'])
        self.assertNotEqual(before['tooling']['sha256'], after['tooling']['sha256'])
        self.assertEqual(fingerprint.compare(before, after, source_only=True), [])
        self.assertEqual({row['group'] for row in fingerprint.compare(before, after, source_only=False)}, {'supporting', 'tooling'})

    def test_embedded_documentation_remains_a_compiler_input(self):
        self.write('runtime/src/lib.rs', 'const README: &str = include_str!("../../vendor/crate/README.md");\n')
        before = self.capture()
        self.write('vendor/crate/README.md', 'Changed embedded compiler input.\n')
        after = self.capture()
        self.assertNotEqual(before['source']['sha256'], after['source']['sha256'])
        self.assertIn('vendor/crate/README.md', {row['path'] for row in after['source']['files']})

    def test_denied_credential_path_is_never_opened(self):
        # Explicit denylist fixture: this file contains no credential material.
        denied = self.write('vendor/fixture/sydneycreds', 'DENYLIST TEST FIXTURE ONLY\n')
        real_open = Path.open
        def guarded_open(path, *args, **kwargs):
            if path == denied:
                raise AssertionError('attempted to open denylisted credential path')
            return real_open(path, *args, **kwargs)
        with patch.object(Path, 'open', guarded_open):
            record = self.capture()
        self.assertIn('vendor/fixture/sydneycreds', {row['path'] for row in record['excludedPaths']})
        self.assertNotIn('vendor/fixture/sydneycreds', {row['path'] for row in record['source']['files']})

    def test_executable_bit_and_binary_changes_are_detected(self):
        path = self.write('scripts/build.sh', '#!/bin/sh\nexit 0\n')
        self.write('node/spec.json', b'\x00\x01\xff')
        before = self.capture()
        path.chmod(0o755)
        self.write('node/spec.json', b'\x00\x02\xff')
        changed = fingerprint.compare(before, self.capture(), source_only=True)
        self.assertEqual({row['path'] for row in changed}, {'scripts/build.sh', 'node/spec.json'})

    def test_symlink_target_must_be_in_inventory(self):
        self.write('runtime/vector.bin', b'fixture')
        os.symlink('vector.bin', self.root / 'runtime/vector-link')
        record = self.capture()
        self.assertEqual(next(row for row in record['source']['files'] if row['path'] == 'runtime/vector-link')['kind'], 'symlink')
        os.symlink('/outside/unbound', self.root / 'runtime/external')
        with self.assertRaisesRegex(RuntimeError, 'External symlink'):
            self.capture()

    def test_policy_and_head_changes_cannot_pass_source_only(self):
        before = self.capture()
        changed = copy.deepcopy(before)
        changed['policySha256'] = 'changed'
        changed['git']['head'] = 'changed'
        self.assertEqual({row['field'] for row in fingerprint.compare(before, changed, True)}, {'policySha256', 'git.head'})

    def test_tracked_historical_root_archive_is_not_a_build_input(self):
        self.write('council-runtime-fixture.zip', b'ARCHIVED RELEASE FIXTURE')
        self.git('add', 'council-runtime-fixture.zip')
        record = self.capture()
        self.assertNotIn('council-runtime-fixture.zip', {row['path'] for row in record['source']['files']})

    def test_changed_recorder_cannot_pass_source_only(self):
        before = self.capture()
        changed = copy.deepcopy(before)
        changed['recorderSha256'] = 'changed'
        self.assertEqual(fingerprint.compare(before, changed, True)[0]['field'], 'recorderSha256')


if __name__ == '__main__':
    unittest.main()
