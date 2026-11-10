"""Synthetic receipts only: no native processes, signatures or crash coverage."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import artifact_identity_release as identity
import forward_repair_release as forward
import maintenance_crash_release as crash
import production_build as build
import release_package as release
import test_forward_repair_release as fixtures


class CrashReleaseTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.ForwardRepairTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.context = self.fixture.prepare()
        self.binary = self.root / 'never-executed-verifier'
        self.binary.write_bytes(b'synthetic verifier, never executed')
        self.binary.chmod(0o555)
        self.sha = identity.sha(self.binary.read_bytes())
        self.serial = 0

    def emit(self, output, pins, name, value):
        path = output / name
        path.parent.mkdir(parents=True, exist_ok=True)
        raw = build.serialized(value) if not isinstance(value, bytes) else value
        path.write_bytes(raw)
        pins[str(path)] = build.digest(path)
        return {'file': name, 'sha256': pins[str(path)]['sha256']}

    def witness(self, output=None, context=None):
        if output is None:
            self.serial += 1
            output = self.root / ('crash-' + str(self.serial))
        context = context or self.context
        coverage, context, _, pins = self.fixture.witness(context, output / 'baseline')
        baseline = coverage['recoveryReplay']
        emit = lambda name, value: self.emit(output, pins, name, value)
        emit('worker-executable', self.binary.read_bytes())
        (output / 'worker-executable').chmod(0o500)
        for version, row in context['wasm'].items():
            emit('inputs/' + row['blake2'] + '.wasm', self.fixture.paths[row['role']].read_bytes())
        for path in sorted((output / 'baseline').glob('snapshot-objects/*')):
            emit('state/snapshot-objects/' + path.name, path.read_bytes())
        for path in sorted((output / 'baseline').glob('snapshots/*')):
            emit('state/snapshots/' + path.name, path.read_bytes())
        prior = {'hash': identity.scale.CHECKPOINT_HASH, 'stateRoot': identity.history.ROOT,
                 'snapshot': {'file': 'state/snapshots/000000.json',
                              'sha256': pins[str(output / 'state/snapshots/000000.json')]['sha256']}}
        workers = []
        for index, block in enumerate([*baseline['blocks'], None]):
            expected = block or prior
            imported = block is not None
            block_pin = engine_pin = None
            if imported:
                block_pin = emit(f'inputs/{block["number"]}.block.scale', (output / 'baseline' / f'{block["number"]}.block.scale').read_bytes())
                engine = context['wasm'][str(context['scenarios']['repair']['steps'][index]['executingSpec'])]
                engine_pin = {'file': 'inputs/' + engine['blake2'] + '.wasm', 'sha256': engine['sha256']}
            job = {'schema': crash.WORKER_SCHEMA, 'index': index, 'parentSnapshot': prior['snapshot'],
                'parentRoot': prior['stateRoot'], 'parentHash': prior['hash'], 'parentNumber': identity.scale.CHECKPOINT + index,
                'block': block_pin, 'engine': engine_pin, 'bootstrapBlake2': context['wasm']['134']['blake2'],
                'deployedBlake2': context['checkpointCodeBlake2'], 'expectedRoot': expected['stateRoot'], 'expectedHash': expected['hash']}
            job_pin = emit(f'jobs/{index:06d}.json', job)
            command = [str(output / 'worker-executable'), 'maintenance-crash-worker', '--job', str(output / f'jobs/{index:06d}.json')]
            prefix = f'stages/{index:06d}/'
            launch = {'schema': 'sora-nexus-maintenance-crash-launch-v1', 'executableSha256': self.sha,
                'jobSha256': job_pin['sha256'], 'command': command, 'deadlineSeconds': crash.DAY, 'productionReady': False}
            started = {'pid': 4242, 'executableSha256': self.sha, 'command': command,
                'jobSha256': job_pin['sha256'], 'deadlineSeconds': crash.DAY,
                'killScope': 'owned importer process; no subprocesses are spawned by the worker'}
            name = f'state/snapshots/{index + int(imported):06d}.json'
            snapshot = {'file': name, 'sha256': pins[str(output / name)]['sha256']}
            ready = {'schema': crash.WORKER_SCHEMA, 'pid': 4242, 'jobSha256': job_pin['sha256'],
                'parentSnapshot': prior['snapshot'], 'snapshot': snapshot, 'root': expected['stateRoot'], 'blockHash': expected['hash'],
                'imported': imported, 'durableSnapshotBarrier': True, 'standaloneEvidenceAdmissible': False}
            stdout = emit(prefix + 'stdout.log', b'')
            stderr = emit(prefix + 'stderr.log', b'')
            process = {'schema': 'sora-nexus-maintenance-process-death-v1', 'pid': 4242,
                'deliberateKillAfterBarrier': True, 'killRequested': True, 'killError': None, 'actualExitCode': None,
                'actualExitSignal': 9, 'elapsedMillis': 1000, 'failure': None, 'jobSha256': job_pin['sha256'],
                'executableSha256': self.sha, 'stdoutSha256': stdout['sha256'], 'stderrSha256': stderr['sha256'],
                'stdoutHashError': None, 'stderrHashError': None}
            workers.append({'index': index, 'kind': 'import' if imported else 'terminal-restore', 'ready': ready, 'process': process,
                'snapshotAuthenticatedAfterKill': True, 'jobReceipt': job_pin, 'launchReceipt': emit(prefix + 'launch.json', launch),
                'startedReceipt': emit(prefix + 'started.json', started), 'readyReceipt': emit(prefix + 'ready.json', ready),
                'processReceipt': emit(prefix + 'process.json', process), 'stdoutReceipt': stdout, 'stderrReceipt': stderr})
            prior = {'hash': expected['hash'], 'stateRoot': expected['stateRoot'], 'snapshot': snapshot}
        final = baseline['blocks'][-1]
        result = {'schema': 'sora-nexus-maintenance-process-death-rehearsal-v1', 'baseline': baseline,
            'incomingReplayBinding': copy.deepcopy(coverage['priorPolicyReplay']), 'executableSha256': self.sha,
            'finalIdentity': {'blockHash': final['hash'], 'stateRoot': final['stateRoot'],
                'activePolicyCommitment': context['incomingContext']['activePolicy']['commitment'],
                'originalPolicyCommitment': context['policyCommitment'], 'phase': 'Complete', 'cursor': final['cursor'],
                'operationCounter': final['operationCounter'], 'runtimeCodeBlake2': context['wasm']['138']['blake2']},
            'workers': workers, 'coverage': crash.COVERAGE, 'allBlocksImportedFromContinuedSnapshots': True,
            'terminalSnapshotRestoredInFreshProcess': True, 'inCommitCrashVerified': False, 'consensusSealsVerified': False,
            'productionReady': False}
        emit('report.json', result)
        return result, context, output, pins, coverage

    def validate(self, value, context, output, pins, coverage):
        return crash.validate(value, context, coverage['priorPolicyReplay'], 1, output, pins, self.sha)

    def test_complete_synthetic_chain_checks_all_receipts_and_scopes(self):
        args = self.witness()
        summary = self.validate(*args)
        self.assertEqual(summary['deliberateSigkills'], len(args[0]['baseline']['blocks']) + 1)
        self.assertEqual(summary['coverageScope'], 'committedSnapshotProcessDeath')
        self.assertFalse(summary['inCommitCrashVerified'])
        self.assertFalse(summary['consensusProcessInterruptionVerified'])
        # Reusing a PID in distinct observed lifetimes is allowed.
        self.assertEqual({row['process']['pid'] for row in args[0]['workers']}, {4242})

    def test_exit_signal_barrier_pid_failure_and_log_admission(self):
        changes = [('process', 'actualExitSignal', 15), ('process', 'actualExitCode', 0),
            ('process', 'pid', 17), ('process', 'deliberateKillAfterBarrier', False),
            ('process', 'killError', 'failed'), ('process', 'failure', 'timeout'),
            ('process', 'stdoutSha256', 'ff' * 32), ('ready', 'durableSnapshotBarrier', False),
            ('ready', 'standaloneEvidenceAdmissible', True), ('ready', 'pid', 17)]
        for field, key, value in changes:
            args = self.witness(); result, _, output, pins, _ = args
            row = result['workers'][1]
            row[field][key] = value
            row[field + 'Receipt'] = self.emit(output, pins, f'stages/000001/{field}.json', row[field])
            self.emit(output, pins, 'report.json', result)
            with self.subTest(field=field, key=key), self.assertRaises(ValueError): self.validate(*args)

    def test_next_process_cannot_reset_to_checkpoint_or_skip_job(self):
        args = self.witness(); result, context, output, pins, _ = args
        row = result['workers'][1]
        job = json.loads((output / row['jobReceipt']['file']).read_text())
        job['parentSnapshot'] = result['workers'][0]['ready']['parentSnapshot']
        row['jobReceipt'] = self.emit(output, pins, row['jobReceipt']['file'], job)
        self.emit(output, pins, 'report.json', result)
        with self.assertRaises(ValueError): self.validate(*args)

    def test_missing_terminal_restore_or_block_worker_rejected(self):
        for which in (-1, 2):
            args = self.witness(); result, _, output, pins, _ = args
            result['workers'].pop(which); self.emit(output, pins, 'report.json', result)
            with self.assertRaises(ValueError): self.validate(*args)

    def test_terminal_cannot_replace_snapshot(self):
        args = self.witness(); result, _, output, pins, _ = args
        row = result['workers'][-1]
        row['ready']['snapshot'] = result['workers'][0]['ready']['snapshot']
        row['readyReceipt'] = self.emit(output, pins, row['readyReceipt']['file'], row['ready'])
        self.emit(output, pins, 'report.json', result)
        with self.assertRaises(ValueError): self.validate(*args)

    def test_binding_final_policy_root_code_and_broader_scope_rejected(self):
        mutations = [lambda r: r['incomingReplayBinding'].update(finalStateRoot='0x' + 'ff' * 32),
            lambda r: r['finalIdentity'].update(activePolicyCommitment='ff' * 32),
            lambda r: r['finalIdentity'].update(runtimeCodeBlake2='ff' * 32),
            lambda r: r.update(inCommitCrashVerified=True), lambda r: r.update(consensusSealsVerified=True),
            lambda r: r['baseline']['priorPolicyIncomingAudits'][0]['finalValidation'].update(pendingIncomingAudits=1)]
        for mutate in mutations:
            args = self.witness(); result, _, output, pins, _ = args
            mutate(result); self.emit(output, pins, 'report.json', result)
            with self.assertRaises(ValueError): self.validate(*args)

    def test_mutated_retained_bytes_worker_code_or_snapshot_fail(self):
        for name in ('stages/000000/ready.json', 'worker-executable', 'state/snapshots/000002.json'):
            args = self.witness(); _, _, output, pins, _ = args
            path = output / name; path.chmod(0o600); path.write_bytes(path.read_bytes() + b'!')
            with self.subTest(name=name), self.assertRaises(ValueError): self.validate(*args)

    def report(self):
        _, context, _, _, coverage = self.witness()
        receipt = {'gate': 'incomingPendingCoverage', 'verifierSha256': self.sha, 'exitCode': 0, 'result': coverage}
        report = {'forwardRepairReplay': context, 'policy': context['originalPolicy'],
            'incomingRecoveryReplay': context['incomingContext'], 'nativeVerifierReceipts': [receipt],
            'artifacts': list(self.fixture.records.values()), 'productionReady': False,
            'gates': {identity.GATE: {'status': 'verified', 'verifierSha256': self.sha},
                forward.GATE: {'status': 'verified', 'verifierSha256': self.sha, 'scenarios': {'repair': {
                    'manifestSha256': context['scenarios']['repair']['manifestSha256'],
                    'replayTranscriptSha256': coverage['priorPolicyReplay']['replayTranscriptSha256'],
                    'blocks': len(context['scenarios']['repair']['steps'])}}},
                'incomingPendingCoverage': {'status': 'verified', 'receipt': receipt},
                crash.GATE: {'status': 'unverified'}, 'fullAndWarpSynchronization': {'status': 'unverified'}}}
        return report

    def test_requires_fresh_same_package_receipt_identity_and_forward_join(self):
        report = self.report()
        crash.prerequisites(report, self.sha)
        report['gates']['incomingPendingCoverage']['receipt'] = copy.deepcopy(report['gates']['incomingPendingCoverage']['receipt'])
        with self.assertRaises(ValueError): crash.prerequisites(report, self.sha)
        report = self.report()
        report['gates'][forward.GATE]['scenarios']['repair']['replayTranscriptSha256'] = 'ff' * 32
        with self.assertRaises(ValueError): crash.prerequisites(report, self.sha)

    def run_mock(self, after_validation=None, mutate_completion=False):
        report = self.report(); paths = dict(self.fixture.paths)
        output = self.root / 'owned-crash-output'
        before = copy.deepcopy(report); paths_before = dict(paths)
        helpers = {str(Path(crash.__file__).resolve()): build.digest(Path(crash.__file__).resolve())}
        source_sha = build.pin_bytes(build.serialized(helpers))['sha256']
        def process(command, cwd, execution_dir, guard, **kwargs):
            self.assertEqual(command, [str(output / 'verifier'), 'rehearse-maintenance-crashes', '--manifest',
                str(output / 'inputs/repair-replay.json'), '--output', str(execution_dir / 'replay'), '--require-prior-incoming-evidence'])
            self.assertGreaterEqual(kwargs['timeout'], (len(self.context['scenarios']['repair']['steps']) + 2) * crash.DAY)
            guard(); execution_dir.mkdir()
            original = self.fixture.paths
            self.fixture.paths = {role: output / 'inputs' / self.fixture.records[role]['sourcePath'] for role in self.context['inputRoles']}
            self.fixture.paths['checkpointCode'] = output / 'inputs' / self.fixture.records['checkpointCode']['sourcePath']
            try: result, _, _, _, _ = self.witness(execution_dir / 'replay', self.context)
            finally: self.fixture.paths = original
            raw = build.serialized(result)
            receipt = {'command': command, 'cwd': str(cwd), 'pid': 999, 'exitCode': 0, 'stopReason': None,
                'guardError': None, 'cleanupErrors': [], 'outputs': {}}
            for name, value in [('stdout.log', raw), ('stderr.log', b''), ('launch.json', {'syntheticOnly': True}), ('started.json', {'pid': 999})]:
                pin = build.write(execution_dir / name, value); kwargs['retain'](execution_dir / name, pin)
                if name in ('stdout.log', 'stderr.log'): receipt['outputs'][name] = pin
            pin = build.write(execution_dir / 'execution.json', receipt); kwargs['retain'](execution_dir / 'execution.json', pin)
            return raw, receipt
        actual_validate = crash.validate
        def validate(*args):
            result = actual_validate(*args)
            if after_validation: after_validation(output, args)
            return result
        actual_tree = crash.tree
        def tree(directory):
            result = actual_tree(directory)
            if mutate_completion and directory == output:
                (output / 'completion.json').write_bytes(b'changed before final registration')
            return result
        with patch.object(crash, 'helper_pins', return_value=helpers), patch.object(build, 'process', side_effect=process), \
             patch.object(crash, 'validate', side_effect=validate), patch.object(crash, 'tree', side_effect=tree):
            try: crash.package_hook(report, paths, self.binary, self.sha, source_sha, output)
            except BaseException:
                self.assertEqual(report, before)
                self.assertEqual(paths, paths_before)
                raise
        self.assertEqual(report['gates'][crash.GATE]['status'], 'verified')
        self.assertEqual(report['gates']['fullAndWarpSynchronization'], before['gates']['fullAndWarpSynchronization'])
        self.assertFalse(report['gates'][crash.GATE]['inCommitCrashVerified'])
        self.assertFalse(report['productionReady'])
        return report

    def test_fixed_fresh_process_and_atomic_scoped_registration(self):
        self.run_mock()

    def test_changed_log_before_registration_does_not_partially_register(self):
        def mutate(output, _):
            with (output / 'execution/stdout.log').open('ab') as stream: stream.write(b' ')
        with self.assertRaises(ValueError): self.run_mock(mutate)

    def test_changed_completion_before_final_guard_does_not_register(self):
        with self.assertRaises(ValueError): self.run_mock(mutate_completion=True)

    def test_absent_opt_in_preserves_package_and_does_not_run_hook(self):
        report = {'artifacts': [], 'gates': {}, 'productionReady': False}
        with patch.object(release, 'verify_candidate', return_value=(report, {})), patch.object(crash, 'package_hook') as hook:
            actual = release.package(self.root / 'unused', self.root / 'ordinary-package')
        self.assertIs(actual, report)
        hook.assert_not_called()
        self.assertNotIn(crash.GATE, actual['gates'])

    def test_partial_opt_in_rejected(self):
        report = {'artifacts': [], 'gates': {}, 'productionReady': False}
        with patch.object(release, 'verify_candidate', return_value=(report, {})):
            with self.assertRaises(ValueError):
                release.package(self.root / 'unused', self.root / 'partial-output', maintenance_crash_source_sha256='ab' * 32)


if __name__ == '__main__':
    unittest.main()
