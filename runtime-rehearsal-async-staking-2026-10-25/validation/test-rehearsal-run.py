#!/usr/bin/env python3
"""Ensure failed/optimized invocations cannot inherit earlier passing evidence."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parent.parent / 'run-local-rehearsals.py'


class RunEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'rehearsal'
        self.validation = self.root / 'validation'
        self.validation.mkdir(parents=True)
        self.old = {'status': 'passed', 'runId': 'old-run'}
        for name in ('rehearsal-run.json', 'fee-event-check.json', 'bridge-readiness.json'):
            (self.validation / name).write_text(json.dumps(self.old))
        self.candidate = self.root / 'candidate-built.wasm'
        self.candidate.write_bytes(b'candidate')
        self.proof = self.validation / 'candidate-build-evidence.json'
        self.proof.write_text(json.dumps({'candidateSha256': 'incorrect'}))

    def test_identity_failure_archives_prior_success_and_writes_fresh_failure(self):
        spec = importlib.util.spec_from_file_location('rehearsal_runner', SOURCE)
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        runner.ROOT, runner.VALIDATION = self.root, self.validation
        with patch.object(sys, 'argv', [str(SOURCE), str(self.candidate), '--build-evidence', str(self.proof)]), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(runner.main(), 1)
        current = json.loads((self.validation / 'rehearsal-run.json').read_text())
        self.assertEqual(current['status'], 'failed')
        self.assertNotEqual(current['runId'], 'old-run')
        self.assertFalse((self.validation / 'fee-event-check.json').exists())
        self.assertFalse((self.validation / 'bridge-readiness.json').exists())
        self.assertEqual(current['stages'], [])
        self.assertEqual(len(current['archivedPriorOutputs']), 3)
        for row in current['archivedPriorOutputs']:
            self.assertEqual(json.loads((self.root / row['path']).read_text()), self.old)
        self.assertTrue(self.proof.exists(), 'Supplied build evidence must not be quarantined')

    def test_optimized_python_writes_failure_instead_of_skipping_assertions(self):
        copy = self.root / SOURCE.name
        shutil.copyfile(SOURCE, copy)
        process = subprocess.run([sys.executable, '-O', str(copy), str(self.candidate), '--build-evidence', str(self.proof)], capture_output=True, text=True)
        self.assertEqual(process.returncode, 1)
        current = json.loads((self.validation / 'rehearsal-run.json').read_text())
        self.assertEqual(current['status'], 'failed')
        self.assertIn('requires Python assertions', current['error']['message'])
        self.assertEqual(current['stages'], [])

    def build_fixture(self):
        spec = importlib.util.spec_from_file_location('rehearsal_proof_runner', SOURCE)
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        def write(name, value):
            path = self.validation / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value))
            return path
        recorder = self.validation / 'source-fingerprint.py'
        recorder.write_text('# synthetic recorder fixture only\n')
        source_hash = 'a' * 64
        fingerprint = {'status': 'final', 'source': {'sha256': source_hash, 'fileCount': 1}, 'recorderSha256': runner.digest(recorder)}
        fingerprint_path = write('fingerprint.json', fingerprint)
        reference = {'path': fingerprint_path.name, 'sha256': runner.digest(fingerprint_path), 'sourceSha256': source_hash, 'sourceFiles': 1}
        proof = {'status': 'passed', 'candidateSha256': 'b' * 64, 'buildCommand': 'synthetic offline fixture', 'sourceInputs': {'source.rs': 'c' * 64}, 'sourceFingerprint': reference, 'verificationReport': 'verification.json'}
        steps = []
        for name in ['source-start', 'clippy-all', 'source-after-clippy', 'format', 'production-feature-graph', 'production-wasm', 'source-after-build']:
            row = {'name': name, 'exitCode': 0}
            for stream in ('stdout', 'stderr'):
                path = self.validation / f'{name}.{stream}.log'
                path.write_text('synthetic offline fixture\n')
                row[stream] = path.name
                row[stream + 'Sha256'] = runner.digest(path)
            steps.append(row)
        verification = {'status': 'passed', 'candidate': proof, 'sourceFingerprint': reference, 'steps': steps}
        write('verification.json', verification)
        for name in ('source-clippy-end-check.json', 'source-build-end-check.json'):
            write(name, {'status': 'passed', 'changes': [], 'compared': 'source', 'sourceSha256': source_hash})
        return runner, proof, verification, write

    def test_complete_build_proof_chain_is_accepted(self):
        runner, proof, _, _ = self.build_fixture()
        fingerprint, files = runner.validate_build_evidence(proof, 'b' * 64, self.validation)
        self.assertEqual(fingerprint.name, 'fingerprint.json')
        self.assertEqual(len(files), 19)

    def test_unpassed_proof_or_changed_fingerprint_is_rejected(self):
        runner, proof, _, _ = self.build_fixture()
        changed = copy.deepcopy(proof)
        changed['status'] = 'failed'
        with self.assertRaises(AssertionError):
            runner.validate_build_evidence(changed, 'b' * 64, self.validation)
        (self.validation / 'fingerprint.json').write_text('{}')
        with self.assertRaises(AssertionError):
            runner.validate_build_evidence(proof, 'b' * 64, self.validation)

    def test_failed_or_missing_build_gate_is_rejected(self):
        for mutation in ('status', 'step', 'missing'):
            runner, proof, verification, write = self.build_fixture()
            if mutation == 'status': verification['status'] = 'failed'
            elif mutation == 'step': verification['steps'][0]['exitCode'] = 1
            else: verification['steps'].pop()
            write('verification.json', verification)
            with self.subTest(mutation=mutation), self.assertRaises(AssertionError):
                runner.validate_build_evidence(proof, 'b' * 64, self.validation)

    def test_changed_source_or_log_is_rejected(self):
        for mutation in ('source', 'log'):
            runner, proof, _, write = self.build_fixture()
            if mutation == 'source': write('source-build-end-check.json', {'status': 'changed', 'changes': ['source.rs']})
            else: (self.validation / 'production-wasm.stdout.log').write_text('changed')
            with self.subTest(mutation=mutation), self.assertRaises(AssertionError):
                runner.validate_build_evidence(proof, 'b' * 64, self.validation)


class CurrentAuditEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repository = Path(self.temp.name)
        self.validation = self.repository / 'rehearsal/validation'
        self.validation.mkdir(parents=True)
        spec = importlib.util.spec_from_file_location('current_audit_runner', SOURCE)
        self.runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.runner)
        collector = self.validation / 'capture-supporting-evidence.py'
        locks = [f'package-{i}/Cargo.lock' for i in range(11)]
        harnesses = [f'harness-{i}' for i in range(5)]
        collector.write_text(f'LOCKFILES = {locks!r}\nHARNESSES = {harnesses!r}\n')
        self.out = self.validation / 'current-cargo-audits'
        self.out.mkdir()
        self.commit = 'a' * 40
        db = {'path': '/synthetic/rustsec', 'commit': self.commit, 'tree': 'b' * 40,
              'clean': True, 'trackedFiles': 2, 'trackedBytesSha256': 'c' * 64}
        database = self.write(self.out / 'database-source.json', {'status': 'passed',
                              'before': db, 'after': db, 'unchanged': True})
        self.report = {'status': 'passed', 'collectorSha256': self.runner.digest(collector),
            'databaseSource': str(database.relative_to(self.repository)),
            'databaseSourceSha256': self.runner.digest(database), 'databaseCommit': self.commit,
            'repositoryLockfiles': 11, 'historicalHarnessLockfiles': 5, 'audits': []}
        inputs = [(self.repository / path, 'repository') for path in locks]
        inputs += [(self.validation / f'imported-migration-evidence/{name}-harness/Cargo.lock',
                    'historical-harness') for name in harnesses]
        for index, (lock, scope) in enumerate(inputs):
            lock.parent.mkdir(parents=True, exist_ok=True)
            lock.write_text('synthetic lockfile\n')
            audit = self.write(self.out / f'{index:02d}.json', {
                'database': {'last-commit': None},
                'vulnerabilities': {'count': 0, 'list': []}, 'warnings': {},
                'settings': {'ignore': [], 'target_arch': [], 'target_os': []}})
            stderr = self.out / f'{index:02d}.stderr.log'
            stderr.write_text('')
            self.report['audits'].append({'scope': scope, 'lockfile': str(lock.relative_to(self.repository)),
                'lockSha256': self.runner.digest(lock), 'audit': str(audit.relative_to(self.repository)),
                'auditSha256': self.runner.digest(audit), 'stderr': str(stderr.relative_to(self.repository)),
                'stderrSha256': self.runner.digest(stderr), 'exit': 0, 'vulnerabilities': 0, 'warnings': 0,
                'databaseCommit': self.commit, 'auditReportedDatabaseCommit': None,
                'command': ['cargo', 'audit', '--db', db['path'], '--no-fetch', '--deny', 'warnings', '--json', '--file', str(lock)]})

    @staticmethod
    def write(path, value):
        path.write_text(json.dumps(value))
        return path

    def verify(self):
        self.write(self.out / 'summary.json', self.report)
        return self.runner.validate_current_cargo_audits(self.validation, self.repository)

    def test_complete_current_audits_allow_null_cli_revision_only_with_independent_database_binding(self):
        _, report = self.verify()
        self.assertEqual(report['databaseCommit'], self.commit)
        self.assertEqual(len(report['audits']), 16)

    def test_missing_duplicate_or_wrong_scope_audit_fails(self):
        pristine = copy.deepcopy(self.report)
        for mutation in ('missing', 'duplicate', 'scope'):
            self.report = copy.deepcopy(pristine)
            if mutation == 'missing': self.report['audits'].pop()
            elif mutation == 'duplicate': self.report['audits'][0] = self.report['audits'][1]
            else: self.report['audits'][0]['scope'] = 'historical-harness'
            with self.subTest(mutation=mutation), self.assertRaises(AssertionError):
                self.verify()

    def test_changed_lock_or_raw_audit_fails(self):
        for field in ('lockfile', 'audit'):
            path = self.repository / self.report['audits'][0][field]
            before = path.read_bytes()
            path.write_text('changed')
            with self.subTest(field=field), self.assertRaises(AssertionError):
                self.verify()
            path.write_bytes(before)

    def test_unbound_database_or_weakened_command_fails(self):
        pristine = copy.deepcopy(self.report)
        for mutation in ('database', 'command', 'reported'):
            self.report = copy.deepcopy(pristine)
            if mutation == 'database': self.report['databaseCommit'] = 'd' * 40
            elif mutation == 'command': self.report['audits'][0]['command'].remove('--deny')
            else: self.report['audits'][0]['auditReportedDatabaseCommit'] = 'd' * 40
            with self.subTest(mutation=mutation), self.assertRaises(AssertionError):
                self.verify()

    def test_ignored_advisories_and_filtered_platforms_fail_even_with_updated_report_hash(self):
        row = self.report['audits'][0]
        path = self.repository / row['audit']
        pristine = json.loads(path.read_text())
        for field in ('ignore', 'target_arch', 'target_os'):
            changed = copy.deepcopy(pristine)
            changed['settings'][field] = ['excluded']
            self.write(path, changed)
            row['auditSha256'] = self.runner.digest(path)
            with self.subTest(field=field), self.assertRaises(AssertionError):
                self.verify()


if __name__ == '__main__':
    unittest.main(verbosity=2)
