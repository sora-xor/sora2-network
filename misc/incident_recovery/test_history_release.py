"""Synthetic hook tests only: no native executable, Wasm or network is run."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import hashi_codec as scale
import history_release as history
import release_package as release
from test_release_package import ReleaseTests, hx, wasm


class HistoryReleaseTests(unittest.TestCase):
    def setUp(self):
        self.fixture = ReleaseTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.headers = []
        parent = hx(20)
        for offset in range(5):
            value = {'parentHash': parent, 'number': hex(scale.CHECKPOINT - 4 + offset), 'stateRoot': hx(30 + offset), 'extrinsicsRoot': hx(40 + offset), 'digest': {'logs': []}}
            parent = history.header(value)['hash']
            self.headers.append(value)
        for target, name, value in [(scale, 'CHECKPOINT_HASH', parent), (history, 'ROOT', hx(34)), (release, 'CHECKPOINT_ROOT', hx(34))]:
            mock = patch.object(target, name, value)
            mock.start()
            self.addCleanup(mock.stop)
        self.fixture.full_fixture()
        self.write('ancestry.json', self.headers)
        self.page = self.write('historical-capture/top.jsonl', b'synthetic top trie page')
        self.child = self.write('historical-capture/child.jsonl', b'synthetic child trie page')
        code = self.write('historical-capture/runtime-code.wasm', b'old-code')
        point = history.header(self.headers[0])
        self.capture = {'schema': 'sora-nexus-checkpoint-capture-v1', 'complete': True, 'checkpoint': point, 'artifacts': {'runtime-code.wasm': release.digest_file(code)}, 'top': {'pageCount': 1, 'pages': [{'file': 'top.jsonl', 'sha256': release.digest_file(self.page)['sha256']}]}, 'children': [{'pageCount': 1, 'pages': [{'file': 'child.jsonl', 'sha256': release.digest_file(self.child)['sha256']}]}]}
        self.write('historical-capture/capture-complete.json', self.capture)
        blocks = []
        for offset, header in enumerate(self.headers[1:]):
            blocks.append(self.ref(self.write(f'blocks/{offset}.json', {'block': {'header': header, 'extrinsics': []}, 'justifications': None})))
        self.manifest = {'schema': 'sora-nexus-historical-replay-v1', 'checkpointHash': scale.CHECKPOINT_HASH, 'candidate': self.ref(self.root / 'bootstrapWasm'), 'ancestry': self.ref(self.root / 'ancestry.json'), 'ranges': [{'name': 'all synthetic boundaries', 'rationale': 'Unit fixture only; no native execution evidence', 'boundaries': sorted(history.BOUNDARIES), 'parentCapture': 'historical-capture', 'blocks': blocks}]}
        self.save_manifest()
        self.binary = self.write('fake-verifier', b'not an executable; Popen must be mocked')
        self.binary_pin = release.digest_file(self.binary)['sha256']

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else json.dumps(value).encode())
        return path

    def ref(self, path):
        return {'file': path.relative_to(self.root).as_posix(), 'sha256': release.digest_file(path)['sha256']}

    def save_manifest(self):
        self.fixture.artifact(history.ROLE, self.manifest, 'history-manifest.json')
        self.fixture.write_candidate()

    def bind(self):
        return release.verify_candidate(self.fixture.candidate)

    def witness(self, report):
        context = report['historicalReplay']
        ranges = []
        for expected in context['ranges']:
            blocks = []
            for index, block in enumerate(expected['blocks']):
                blocks.append({**block, 'deployedExecutorHash': hx(77), 'candidateExecutorHash': context['candidateBlake2'], 'deployedSpec': 134, 'substituteApplied': True, 'boundaries': [['session'], ['epoch'], ['upgrade'], ['checkpoint']][index], 'identicalStateRoots': True, 'nativeExecution': False})
            ranges.append({key: expected[key] for key in ['name', 'rationale', 'parentCaptureManifestSha256']} | {'boundaries': sorted(history.BOUNDARIES), 'blocks': blocks})
        return {key: context[key] for key in ['manifestSha256', 'candidateSha256', 'candidateBlake2', 'ancestrySha256']} | {'schema': 'sora-nexus-historical-replay-witness-v1', 'checkpointHash': scale.CHECKPOINT_HASH, 'ranges': ranges, 'representativeBoundaryCoverage': True, 'consensusSealsVerified': False, 'productionReady': False}

    def process(self, witness, *, code=0, mutate=None, raw=None):
        def popen(command, **options):
            self.assertEqual(command[1:3], ['replay-history', '--manifest'])
            self.assertEqual(Path(command[0]).name, 'verifier')
            self.assertEqual(options['env'], {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'})
            self.assertNotEqual(Path(command[-1]), self.root / 'history-manifest.json')
            options['stdout'].write(json.dumps(witness).encode() if raw is None else raw)
            options['stdout'].flush()
            if mutate:
                mutate(command)
            class Fake:
                pid = 12345
                returncode = code
                def poll(self):
                    return self.returncode
                def wait(self):
                    return self.returncode
            return Fake()
        return popen

    def run_hook(self, report, paths, **kwargs):
        witness = kwargs.pop('witness', self.witness(report))
        with patch.object(history.subprocess, 'Popen', side_effect=self.process(witness, **kwargs)) as invoked:
            history.run(report, paths, self.binary, self.binary_pin, self.root, release.local_file, release.digest_file)
        return invoked

    def test_complete_graph_bound_and_only_history_gate_clears(self):
        report, paths = self.bind()
        self.assertIn(self.child.resolve(), paths.values())
        before = copy.deepcopy(report['gates'])
        invoked = self.run_hook(report, paths)
        self.assertEqual(invoked.call_count, 1)
        self.assertEqual(report['gates'][history.GATE]['status'], 'verified')
        self.assertFalse(report['productionReady'])
        for gate in before:
            if gate != history.GATE:
                self.assertEqual(report['gates'][gate], before[gate])
        directory = Path(report['historicalReplayOutputDirectory'])
        self.assertEqual(json.loads((directory / 'execution.json').read_text())['pid'], 12345)
        self.assertEqual(json.loads((directory / 'guard.json').read_text())['inputChanges'], [])
        self.assertEqual((directory / 'verifier').stat().st_mode & 0o777, 0o555)
        self.assertTrue(any(row['sourcePath'].endswith('/execution.json') for row in report['artifacts']))

    def test_archived_success_without_executable_cannot_clear_gate(self):
        self.fixture.artifact('historicalReplayEvidence', {'productionReady': True, 'representativeBoundaryCoverage': True})
        self.fixture.write_candidate(evidence=[{'gate': history.GATE, 'artifacts': ['historicalReplayEvidence']}])
        report = release.package(self.fixture.candidate, self.root / 'bundle')
        self.assertEqual(report['gates'][history.GATE]['status'], 'unverified')
        self.assertNotIn('historicalReplayOutputDirectory', report)

    def test_package_hook_uses_fresh_process_and_retains_bundle_receipts(self):
        report, _ = self.bind()
        with patch.object(release, 'run_evidence_verifier'), patch.object(history.subprocess, 'Popen', side_effect=self.process(self.witness(report))) as invoked:
            result = release.package(self.fixture.candidate, self.root / 'bundle', verifier=self.binary, verifier_sha256=self.binary_pin)
        self.assertEqual(invoked.call_count, 1)
        self.assertEqual(result['gates'][history.GATE]['status'], 'verified')
        self.assertFalse(result['productionReady'])
        for row in result['artifacts']:
            self.assertEqual(release.digest_file(self.root / 'bundle' / row['object'])['sha256'], row['sha256'])

    def test_manifest_candidate_must_be_packaged_bootstrap(self):
        self.manifest['candidate'] = self.ref(self.write('different.wasm', wasm(134, 131, b'changed')))
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'packaged bootstrap'):
            self.bind()

    def test_public_fixture_provenance_rejected(self):
        path = self.root / 'bootstrapProvenance'
        value = json.loads(path.read_text()) | {'fixtureOnly': True}
        self.fixture.artifact('bootstrapProvenance', value)
        self.fixture.write_candidate()
        with self.assertRaisesRegex(ValueError, 'fixture'):
            self.bind()

    def test_nested_child_hash_mismatch(self):
        self.child.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            self.bind()

    def test_duplicate_capture_reference(self):
        self.capture['children'][0]['pages'][0]['file'] = 'top.jsonl'
        self.write('historical-capture/capture-complete.json', self.capture)
        with self.assertRaisesRegex(ValueError, 'duplicate capture'):
            self.bind()

    def test_traversal_and_symlink_rejected(self):
        self.manifest['ancestry']['file'] = '../ancestry.json'
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'under candidate'):
            self.bind()
        self.manifest['ancestry'] = self.ref(self.root / 'ancestry.json')
        self.save_manifest()
        original = self.child.read_bytes()
        target = self.write('child-original', original)
        self.child.unlink()
        self.child.symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'Symlink'):
            self.bind()

    def test_ancestry_and_parent_root_mismatch(self):
        self.capture['checkpoint']['stateRoot'] = hx(99)
        self.write('historical-capture/capture-complete.json', self.capture)
        with self.assertRaisesRegex(ValueError, 'parent identity'):
            self.bind()

    def test_declared_and_observed_boundaries_both_required(self):
        report, _ = self.bind()
        result = self.witness(report)
        result['ranges'][0]['blocks'][1]['boundaries'] = []
        result['ranges'][0]['boundaries'].remove('epoch')
        with self.assertRaisesRegex(ValueError, 'not observed'):
            history.validate(result, report['historicalReplay'])
        self.manifest['ranges'][0]['boundaries'].remove('upgrade')
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'representative'):
            self.bind()

    def test_wrong_roots_modes_identity_and_scope_rejected(self):
        report, _ = self.bind()
        for field, value in [('postStateRoot', hx(90)), ('candidateExecutorHash', hx(91)), ('nativeExecution', True), ('identicalStateRoots', False), ('substituteApplied', False), ('deployedSpec', True)]:
            with self.subTest(field=field):
                result = self.witness(report)
                result['ranges'][0]['blocks'][0][field] = value
                with self.assertRaises(ValueError):
                    history.validate(result, report['historicalReplay'])
        for field, value in [('consensusSealsVerified', True), ('productionReady', True), ('representativeBoundaryCoverage', False), ('manifestSha256', '0' * 64)]:
            with self.subTest(field=field):
                result = self.witness(report)
                result[field] = value
                with self.assertRaises(ValueError):
                    history.validate(result, report['historicalReplay'])

    def test_original_input_mutation_before_execution_rejected(self):
        report, paths = self.bind()
        self.child.write_bytes(b'changed after bind')
        with patch.object(history.subprocess, 'Popen') as invoked, self.assertRaisesRegex(ValueError, 'before replay'):
            history.run(report, paths, self.binary, self.binary_pin, self.root, release.local_file, release.digest_file)
        invoked.assert_not_called()

    def test_mutation_during_process_cannot_clear_gate(self):
        for what in ['original', 'owned', 'verifier']:
            with self.subTest(what=what):
                report, paths = self.bind()
                def mutate(command):
                    path = self.child if what == 'original' else (Path(command[0]) if what == 'verifier' else Path(command[-1]).parent / 'historical-capture/child.jsonl')
                    path.chmod(0o644)
                    path.write_bytes(b'changed while subprocess ran')
                try:
                    with self.assertRaisesRegex(ValueError, 'changed'):
                        self.run_hook(report, paths, mutate=mutate)
                    self.assertEqual(report['gates'][history.GATE]['status'], 'unverified')
                    self.assertTrue((Path(report['historicalReplayOutputDirectory']) / 'guard.json').is_file())
                finally:
                    self.child.write_bytes(b'synthetic child trie page')

    def test_process_failure_and_invalid_or_oversized_output_retained(self):
        for kwargs in [{'code': 7}, {'raw': b'not json'}, {'raw': b'x' * (history.MAX_OUTPUT + 1)}]:
            with self.subTest(kwargs=list(kwargs)):
                report, paths = self.bind()
                with self.assertRaises((ValueError, json.JSONDecodeError)):
                    self.run_hook(report, paths, **kwargs)
                directory = Path(report['historicalReplayOutputDirectory'])
                self.assertTrue((directory / 'execution.json').is_file())
                self.assertTrue((directory / 'guard.json').is_file())
                self.assertEqual(report['gates'][history.GATE]['status'], 'unverified')

    def test_explicit_verifier_pin_required(self):
        report, paths = self.bind()
        with patch.object(history.subprocess, 'Popen') as invoked, self.assertRaisesRegex(ValueError, 'verifier pin'):
            history.run(report, paths, self.binary, '0' * 64, self.root, release.local_file, release.digest_file)
        invoked.assert_not_called()

    def test_duplicate_json_and_additional_report_fields_rejected(self):
        with self.assertRaisesRegex(ValueError, 'duplicate JSON'):
            history.parse(b'{"a": 1, "a": 2}')
        report, _ = self.bind()
        with self.assertRaisesRegex(ValueError, 'report fields'):
            history.validate(self.witness(report) | {'nativeExecution': True}, report['historicalReplay'])

    def test_timeout_kills_mock_process_and_preserves_failure(self):
        report, paths = self.bind()
        class Pending:
            pid = 12345
            returncode = None
            def poll(self):
                return self.returncode
            def wait(self):
                self.returncode = -9
                return self.returncode
        with patch.object(history.subprocess, 'Popen', return_value=Pending()), patch.object(history.time, 'monotonic', side_effect=[0, 10801, 10802]), patch.object(history.os, 'killpg') as killed:
            with self.assertRaisesRegex(ValueError, 'native process failed'):
                history.run(report, paths, self.binary, self.binary_pin, self.root, release.local_file, release.digest_file)
        killed.assert_called_once_with(12345, history.signal.SIGKILL)
        directory = Path(report['historicalReplayOutputDirectory'])
        self.assertEqual(json.loads((directory / 'execution.json').read_text())['stopReason'], 'timeout')
        self.assertEqual(report['gates'][history.GATE]['status'], 'unverified')

    def test_launch_failure_preserves_receipts_without_false_pid(self):
        report, paths = self.bind()
        with patch.object(history.subprocess, 'Popen', side_effect=OSError('synthetic launch failure')):
            with self.assertRaisesRegex(ValueError, 'native process failed'):
                history.run(report, paths, self.binary, self.binary_pin, self.root, release.local_file, release.digest_file)
        directory = Path(report['historicalReplayOutputDirectory'])
        receipt = json.loads((directory / 'execution.json').read_text())
        self.assertIsNone(receipt['pid'])
        self.assertIsNone(receipt['exitCode'])
        self.assertFalse((directory / 'started.json').exists())

    def test_busy_history_process_survives_the_old_one_hour_limit(self):
        report, paths = self.bind()
        base = self.process(self.witness(report))
        def delayed(command, **options):
            process = base(command, **options)
            polls = iter([None, 0, 0])
            process.poll = lambda: next(polls)
            return process
        with patch.object(history.subprocess, 'Popen', side_effect=delayed), patch.object(history.time, 'monotonic', side_effect=[0, 3601, 3602]), patch.object(history.time, 'sleep'), patch.object(history.os, 'killpg') as killed:
            history.run(report, paths, self.binary, self.binary_pin, self.root, release.local_file, release.digest_file)
        killed.assert_not_called()
        self.assertEqual(report['gates'][history.GATE]['status'], 'verified')

    def test_guard_to_binding_mutation_never_registers_outputs(self):
        for name in ['stdout.json', 'execution.json', 'started.json', 'verifier', 'inputs/historical-capture/child.jsonl']:
            with self.subTest(name=name):
                report, paths = self.bind()
                before_artifacts = copy.deepcopy(report['artifacts'])
                before_paths = dict(paths)
                original = history.write
                def write_then_mutate(path, value):
                    result = original(path, value)
                    if path.name == 'guard.json':
                        target = path.parent / name
                        target.chmod(0o644)
                        target.write_bytes(b'changed after validation and guard')
                    return result
                with patch.object(history, 'write', side_effect=write_then_mutate):
                    with self.assertRaisesRegex(ValueError, 'changed before binding'):
                        self.run_hook(report, paths)
                self.assertEqual(report['artifacts'], before_artifacts)
                self.assertEqual(paths, before_paths)
                self.assertEqual(report['gates'][history.GATE]['status'], 'unverified')

    def test_history_owns_fresh_directory_separate_from_incoming(self):
        report, paths = self.bind()
        incoming = self.root / '.sora-nexus-incoming-replay-existing'
        incoming.mkdir()
        marker = incoming / 'keep.json'
        marker.write_text('existing incoming evidence')
        report['incomingReplayOutputDirectory'] = str(incoming)
        self.run_hook(report, paths)
        directory = Path(report['historicalReplayOutputDirectory'])
        self.assertNotEqual(directory, incoming.resolve())
        self.assertTrue(directory.name.startswith('.sora-nexus-history-replay-'))
        self.assertEqual(marker.read_text(), 'existing incoming evidence')

    def test_unexpected_generated_file_rejected(self):
        report, paths = self.bind()
        def mutate(command):
            (Path(command[0]).parent / 'unrequested-output').write_text('native replay should only write stdout/stderr')
        with self.assertRaisesRegex(ValueError, 'unexpected generated file'):
            self.run_hook(report, paths, mutate=mutate)


if __name__ == '__main__':
    unittest.main()
