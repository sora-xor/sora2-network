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


if __name__ == '__main__':
    unittest.main(verbosity=2)
