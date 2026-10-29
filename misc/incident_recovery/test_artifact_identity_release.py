"""Synthetic packaging joins only: no native recovery, Cargo or Wasm execution."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import artifact_identity_release as identity
import production_build
import release_package as release
import hashi_codec as scale
from test_release_package import policy, wasm, hx


class ArtifactIdentityTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='artifact-identity-unit-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.records, self.paths = {}, {}
        for role, spec, tx in [('bootstrapWasm', 134, 131), ('holdWasm', 135, 132), ('finalWasm', 138, 134)]:
            self.put(role, wasm(spec, tx))
        self.put('policyScale', policy())
        self.put('operatorsScale', b'synthetic operator inventory; outer packager verifies it')
        for role in ['bootstrapBinding', 'holdBinding']:
            self.put(role, {'syntheticOnly': True})
        for role in ['bootstrapProvenance', 'holdProvenance', 'finalProvenance']:
            self.put(role, {'schema': 'sora-nexus-production-unit-placeholder-v1', 'fixtureOnly': False})
        self.chain = {'recovery': {'previousGrandpaSetId': 44, 'firstRecoverySlot': 101,
            'approvedGrandpaKeys': [hx(9)], 'grandpaAuthorities': [[hx(9), 1]]}}
        self.put('chainSpec', self.chain)
        deployed = self.put('checkpointCode', wasm(134, 131, b'deployed'), 'capture/runtime-code.wasm')
        capture = {'schema': 'sora-nexus-checkpoint-capture-v1', 'complete': True,
            'checkpoint': {'hash': scale.CHECKPOINT_HASH, 'stateRoot': identity.history.ROOT},
            'artifacts': {'runtime-code.wasm': {'sha256': self.records['checkpointCode']['sha256'], 'bytes': deployed.stat().st_size}},
            'top': {'pages': []}, 'children': []}
        self.put('checkpointCaptureManifest', capture, 'capture/capture-complete.json')
        self.put('terminal', b'snxrec01\x01\x05synthetic-terminal', 'terminal.scale')
        self.put('escape', b'snxrec01\x01\x04synthetic-authorization', 'escape.scale')
        self.put('tick', b'snxrec01\x01\x00', 'tick.scale')
        base = {'checkpointHash': scale.CHECKPOINT_HASH, 'checkpointCapture': 'capture',
            'policy': self.ref('policyScale'), 'bootstrap': self.ref('bootstrapWasm'), 'hold': self.ref('holdWasm'),
            'finalRuntime': self.ref('finalWasm'), 'previousGrandpaSet': 44, 'freshGrandpa': [[hx(9), 1]]}
        self.activation = {**copy.deepcopy(base), 'schema': 'sora-nexus-maintenance-replay-v1',
            'steps': [self.step(101, 'terminal', 138, 'Complete'), self.step(102, None, 138, 'Complete')]}
        self.hold = {**copy.deepcopy(base), 'schema': 'sora-nexus-supplied-council-maintenance-replay-v1',
            'steps': [self.step(101, 'tick', 134, 'Applying'), self.step(102, 'escape', 135, 'Hold'), self.step(103, 'tick', 135, 'Hold')],
            'suppliedCouncil': {'escapes': [{'step': 1, 'envelopeSha256': self.records['escape']['sha256'], 'destinationSpec': 135}],
                'expectedTerminal': {'specVersion': 135, 'phase': 'Hold', 'cursor': 0, 'auditCursor': 0,
                    'operationCounter': 1, 'ordinaryBlocks': 0, 'activePolicyCommitment': '0x' + identity.blake(policy()),
                    'stateRoot': hx(88), 'terminalAuditBlake2': None}}}
        self.save_manifests()

    def put(self, role, value, name=None, executable=False):
        path = self.root / (name or role)
        path.parent.mkdir(parents=True, exist_ok=True)
        raw = json.dumps(value, sort_keys=True).encode() if isinstance(value, dict) else value
        path.write_bytes(raw)
        if executable:
            path.chmod(0o555)
        pin = release.digest_file(path)
        self.paths[role] = path
        self.records[role] = {'role': role, **pin, 'object': 'objects/' + pin['sha256'],
            'sourcePath': path.relative_to(self.root).as_posix(), 'executable': executable}
        return path

    def ref(self, role):
        return {'file': self.paths[role].relative_to(self.root).as_posix(), 'sha256': self.records[role]['sha256']}

    def step(self, slot, role, spec, phase):
        return {'slot': slot, 'timestamp': slot * 6000, 'authorityIndex': 0,
            'recovery': self.ref(role) if role else None, 'expectedSpec': spec, 'expectedPhase': phase}

    def save_manifests(self):
        self.put('artifactActivationReplay', self.activation, 'activation.json')
        self.put('artifactHoldReplay', self.hold, 'hold.json')

    def bind(self):
        return identity.bind(self.root, self.records, self.paths, release.local_file, release.digest_file)

    def dynamic(self):
        self.put('incoming', b'snxrec01\x01\x06' + scale.vec(scale.u32(0) + b'operation'), 'incoming.scale')
        self.activation['steps'] = [self.step(101, 'incoming', 134, 'Applying'),
            self.step(102, 'terminal', 138, 'Complete'), self.step(103, None, 138, 'Complete')]
        self.put('nativeExecutable', b'synthetic bytes; never executed', 'native-recorder', executable=True)
        binding = {'schema': 'sora-nexus-native-operation-engine-binding-v1', 'fixtureOnly': False, 'productionReady': False,
            'engineSha256': self.records['bootstrapWasm']['sha256'], 'engineBlake2': self.records['bootstrapWasm']['blake2_256'],
            'policySha256': self.records['policyScale']['sha256'], 'policyCommitment': identity.blake(policy()),
            'sdk': 'polkadot-stable2603-3', 'specVersion': 134, 'transactionVersion': 131,
            'runtimeFeatureProfile': {}, 'hostLockAmendment': {}, 'sdkCompilationRepair': {}}
        self.put('nativeBinding', binding, 'engine-binding.json')
        self.activation['nativeOperationRecorder'] = {'executable': self.ref('nativeExecutable'), 'engineBinding': self.ref('nativeBinding')}
        self.save_manifests()
        return binding

    def test_pair_binds_exact_codes_controls_and_execution_sequence(self):
        context = self.bind()
        self.assertEqual(context['scenarios']['activation']['steps'][-1]['executingSpec'], 138)
        self.assertEqual(context['scenarios']['hold']['steps'][-1]['executingSpec'], 135)
        self.assertEqual(context['wasm']['134']['sha256'], self.records['bootstrapWasm']['sha256'])
        self.assertEqual(context['policyCommitment'], identity.blake(policy()))
        self.assertFalse(context['productionReady'])
        self.assertIn('checkpointCode', context['inputRoles'])

    def test_absent_pair_is_optional_and_half_pair_fails(self):
        for role in identity.ROLES:
            self.paths.pop(role)
        self.assertIsNone(self.bind())
        self.save_manifests()
        self.paths.pop('artifactHoldReplay')
        with self.assertRaisesRegex(ValueError, 'both replay scenarios'):
            self.bind()

    def test_installing_a_runtime_without_executing_it_is_insufficient(self):
        original = copy.deepcopy(self.activation)
        self.activation['steps'] = self.activation['steps'][:1]
        self.save_manifests()
        with self.assertRaises(ValueError): self.bind()
        self.activation = original
        self.activation['steps'] = [self.step(101, 'tick', 134, 'Applying'), self.step(102, 'terminal', 138, 'Complete')]
        self.save_manifests()
        with self.assertRaisesRegex(ValueError, 'subsequent final138'): self.bind()
        self.activation = original
        self.hold['steps'] = self.hold['steps'][:1]
        self.save_manifests()
        with self.assertRaises(ValueError): self.bind()

    def test_operator_set_first_slot_and_capture_must_match_package(self):
        original = copy.deepcopy(self.hold)
        mutations = [lambda m: m.update(previousGrandpaSet=45), lambda m: m.update(freshGrandpa=[[hx(10), 1]]),
            lambda m: m['steps'][0].update(slot=100), lambda m: m.update(checkpointCapture='other-capture')]
        for change in mutations:
            self.hold = copy.deepcopy(original); change(self.hold); self.save_manifests()
            with self.assertRaises((ValueError, FileNotFoundError)): self.bind()

    def test_same_version_other_wasm_or_policy_bytes_rejected(self):
        self.put('otherBootstrap', wasm(134, 131, b'changed'), 'other-bootstrap.wasm')
        self.activation['bootstrap'] = self.ref('otherBootstrap'); self.save_manifests()
        with self.assertRaisesRegex(ValueError, 'differs from package'): self.bind()
        self.activation['bootstrap'] = self.ref('bootstrapWasm')
        self.put('otherPolicy', policy()[:-1] + b'\xff', 'other-policy.scale')
        self.hold['policy'] = self.ref('otherPolicy'); self.save_manifests()
        with self.assertRaisesRegex(ValueError, 'differs from package'): self.bind()

    def test_fixture_fields_and_repair_paths_rejected_even_if_null_or_false(self):
        original = copy.deepcopy(self.activation)
        for field, value in [('fixtureCouncil', None), ('publicDynamicRepair', None), ('repairs', []), ('nativeOperationRecordersV2', [])]:
            self.activation = {**copy.deepcopy(original), field: value}; self.save_manifests()
            with self.assertRaisesRegex(ValueError, 'fixture/repair/unknown'): self.bind()
        self.activation = original
        self.activation['steps'][0]['fixtureHashi'] = False; self.save_manifests()
        with self.assertRaisesRegex(ValueError, 'fixture/unknown step'): self.bind()

    def test_fixture_provenance_or_known_public_key_across_roles_rejected(self):
        self.put('finalProvenance', {'fixtureOnly': True})
        with self.assertRaisesRegex(ValueError, 'fixture source'): self.bind()
        self.put('finalProvenance', {'fixtureOnly': False})
        public = bytes.fromhex(next(iter(production_build.PUBLIC_SIGNING_KEYS)))
        self.put('policyScale', policy()[:101] + public + policy()[133:])
        with self.assertRaisesRegex(ValueError, 'known public signing'): self.bind()
        self.put('policyScale', policy())
        self.chain['recovery']['approvedGrandpaKeys'] = ['0x' + public.hex()]
        self.put('chainSpec', self.chain)
        with self.assertRaisesRegex(ValueError, 'known public signing'): self.bind()

    def test_hold_escape_scope_counter_policy_and_steps_are_exact(self):
        original = copy.deepcopy(self.hold)
        for mutate in [lambda m: m['suppliedCouncil']['expectedTerminal'].update(operationCounter=2),
            lambda m: m['suppliedCouncil']['expectedTerminal'].update(activePolicyCommitment=hx(4)),
            lambda m: m['suppliedCouncil']['escapes'][0].update(destinationSpec=136),
            lambda m: m['steps'][1].update(recovery=self.ref('terminal')),
            lambda m: m['steps'][0].update(rejectPreviousEscapes=True)]:
            self.hold = copy.deepcopy(original); mutate(self.hold); self.save_manifests()
            with self.assertRaises(ValueError): self.bind()

    def test_capture_graph_missing_changed_or_aliased_dependencies_rejected(self):
        runtime = self.paths['checkpointCode']
        original = runtime.read_bytes(); runtime.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'digest differs'): self.bind()
        runtime.write_bytes(original)
        capture = json.loads(self.paths['checkpointCaptureManifest'].read_bytes())
        capture['artifacts'] = {}; self.put('checkpointCaptureManifest', capture, 'capture/capture-complete.json')
        with self.assertRaisesRegex(ValueError, 'empty checkpoint graph'): self.bind()

    def test_native_recorder_required_only_for_typed_dynamic_work(self):
        self.dynamic()
        context = self.bind()
        self.assertEqual(context['scenarios']['activation']['dynamicSteps'], [0])
        self.assertEqual(context['scenarios']['activation']['steps'][0]['operationIndex'], 0)
        self.activation.pop('nativeOperationRecorder'); self.save_manifests()
        with self.assertRaisesRegex(ValueError, 'dynamic work requires'): self.bind()

    def test_native_recorder_exact_pins_policy_and_mode(self):
        binding = self.dynamic()
        binding['policyCommitment'] = 'ff' * 32
        self.put('nativeBinding', binding, 'engine-binding.json')
        self.activation['nativeOperationRecorder']['engineBinding'] = self.ref('nativeBinding'); self.save_manifests()
        with self.assertRaisesRegex(ValueError, 'native recorder binding differs'): self.bind()
        binding['policyCommitment'] = identity.blake(policy())
        self.put('nativeBinding', binding, 'engine-binding.json')
        self.activation['nativeOperationRecorder']['engineBinding'] = self.ref('nativeBinding'); self.save_manifests()
        self.paths['nativeExecutable'].chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'mode555'): self.bind()

    def witness(self, scenario='activation', context=None, output=None):
        """Synthetic hash closure, deliberately not evidence of runtime execution."""
        context = copy.deepcopy(context or self.bind())
        context['inputPaths'] = {role: str(path) for role, path in self.paths.items()}
        output = output or self.root / ('synthetic-' + scenario)
        output.mkdir(parents=True)
        plan = context['scenarios'][scenario]
        pins = {str(path): release.digest_file(path) for path in self.paths.values()}
        def emit(name, value):
            path = output / name
            path.parent.mkdir(parents=True, exist_ok=True)
            raw = json.dumps(value, separators=(',', ':')).encode() if isinstance(value, dict) else value
            path.write_bytes(raw)
            pins[str(path)] = release.digest_file(path)
            return pins[str(path)]['sha256']
        base = b'synthetic compressed trie; never restored by a runtime'
        base_sha = identity.sha(base)
        base_ref = {'file': 'snapshot-objects/' + base_sha + '.scale.gz', 'sha256': base_sha}
        emit(base_ref['file'], base)
        def snapshot(index, root, parent):
            obj = {'schema': 'sora-nexus-maintenance-snapshot-v1', 'index': index, 'parent': parent,
                'base': base_ref, 'delta': base_ref if index else None, 'root': root,
                'rawSnapshotSha256': identity.sha(b'synthetic raw snapshot')}
            name = f'snapshots/{index:06d}.json'
            return {'file': name, 'sha256': emit(name, obj)}
        previous_snapshot = snapshot(0, identity.history.ROOT, None)
        parent = scale.CHECKPOINT_HASH
        rows = []
        for index, detail in enumerate(plan['steps']):
            step = detail['step']; version = detail['executingSpec']; number = scale.CHECKPOINT + index + 1
            state = hx(88 if scenario == 'hold' else 90 + index)
            lineage = scale.MAGIC + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + bytes.fromhex(context['policyCommitment'])
            logs = [b'\x06BABE' + scale.vec(b'\x02' + scale.u32(step['authorityIndex']) + step['slot'].to_bytes(8, 'little')),
                b'\x00' + scale.vec(lineage)]
            header = bytes.fromhex(parent[2:]) + scale.compact(number) + bytes.fromhex(state[2:]) + bytes(32) + scale.compact(len(logs)) + b''.join(logs)
            extrinsics = [b'\x04\x01\x00' + scale.compact(step['timestamp'])]
            if detail['recoveryRole']:
                extrinsics.append(b'\x04\x00\x00' + scale.vec(Path(context['inputPaths'][detail['recoveryRole']]).read_bytes()))
            raw = header + scale.compact(len(extrinsics)) + b''.join(scale.vec(x) for x in extrinsics)
            block_sha = emit(f'{number}.block.scale', raw)
            previous_snapshot = snapshot(index + 1, state, previous_snapshot)
            row = {'number': number, 'hash': '0x' + identity.blake(header), 'stateRoot': state,
                'executingCodeBlake2': context['wasm'][str(version)]['blake2'], 'resultingSpec': step['expectedSpec'],
                'phase': step['expectedPhase'], 'cursor': context['workChunks'] if scenario == 'activation' else 0,
                'operationCounter': 1 if scenario == 'hold' and index >= 1 else 0, 'blockSha256': block_sha,
                'diskSnapshotManifestSha256': previous_snapshot['sha256'],
                'forbiddenExtrinsicsRejected': 0 if version == 138 else 2 + len(detail['negativeRoles']) + (1 if scenario == 'hold' and index > 1 else 0),
                'maintenanceSessionFrozen': version != 138, 'independentImportRootMatches': True,
                'diskRestartRootMatches': True, 'generatedAuthorizationSha256': None}
            rows.append(row); parent = row['hash']
        result = {'schema': 'sora-nexus-' + ('maintenance' if scenario == 'activation' else 'supplied-council-maintenance') + '-replay-witness-v1',
            'manifestSha256': plan['manifestSha256'], 'captureManifestSha256': context['captureManifestSha256'],
            'policyCommitment': context['policyCommitment'], 'artifactSha256': plan['expectedArtifacts'], 'blocks': rows,
            'automaticActivationObserved': scenario == 'activation', 'councilHoldObserved': scenario == 'hold',
            'forwardRepairObserved': False, 'ordinaryRuntimeBlocks': 1 if scenario == 'activation' else 0,
            'livenessGraceObserved': False, 'feeClockShiftObserved': scenario == 'activation', 'bridgeHaltRetained': True,
            'nativeExecution': False, 'consensusSealsVerified': False, 'operatorApprovalVerified': False,
            'productionReady': False, 'publicTestCouncilFixture': False, 'publicTestHashiFixture': False,
            'fixtureGovernanceStateUnchanged': None, 'fixtureStateExpectationChecks': 0,
            'nativeOperationDerivations': [], 'repairedNativeOperationDerivations': []}
        if scenario == 'hold':
            prefix = f'{scale.CHECKPOINT + 2}.supplied.'
            auth = {'step': 1, 'blockNumber': scale.CHECKPOINT + 2, 'parentHash': rows[0]['hash'], 'recoveryAnchor': rows[0]['hash'],
                'envelopeSha256': emit(prefix + 'escape.scale', self.paths['escape'].read_bytes()),
                'authorizationSha256': emit(prefix + 'authorization.scale', b'synthetic authorization'),
                'signingMessageSha256': emit(prefix + 'signing-message.scale', b'synthetic signing message'),
                'phase': 'Applying', 'cursor': 0, 'operationCounter': 0,
                'storedCodeBlake2': '0x' + context['checkpointCodeBlake2'],
                'executingCodeBlake2': '0x' + context['wasm']['134']['blake2'],
                'destinationCodeBlake2': '0x' + context['wasm']['135']['blake2'], 'destinationSpec': 135,
                'revisedPolicyCommitment': hx(0), 'signers': ['0x' + key for key in context['council']]}
            emit(prefix + 'authorization.json', auth)
            result['suppliedCouncil'] = {'schema': 'sora-nexus-supplied-council-replay-witness-v1',
                'authorizations': [auth], 'terminal': plan['manifest']['suppliedCouncil']['expectedTerminal'],
                'signaturesVerifiedAgainstPinnedPolicy': True, 'automaticActivationObserved': False,
                'publicTestCouncilGenerated': False, 'councilInventoryApprovalVerified': False,
                'sourceProvenanceVerified': False, 'productionReady': False, 'publicDynamicFixture': False}
        self.transcript(result)
        return result, context, output, pins

    def transcript(self, result):
        transcript = identity.sha(b'sora-nexus-maintenance-transcript-v1' + result['manifestSha256'].encode() + result['captureManifestSha256'].encode())
        for row in result['blocks']:
            transcript = identity.sha(transcript.encode() + json.dumps({key: row[key] for key in identity.BLOCK_FIELDS}, separators=(',', ':')).encode())
        result['replayTranscriptSha256'] = transcript

    def test_validate_exact_synthetic_activation_and_hold_closures(self):
        for scenario in ('activation', 'hold'):
            result, context, output, pins = self.witness(scenario)
            identity.validate(result, context, scenario, output, pins)

    def test_validate_rejects_changed_report_even_with_rehashed_transcript(self):
        result, context, output, pins = self.witness()
        mutations = [lambda r: r['blocks'][1].update(executingCodeBlake2=context['wasm']['134']['blake2']),
            lambda r: r['blocks'][0].update(operationCounter=1), lambda r: r['blocks'][0].update(independentImportRootMatches=False),
            lambda r: r.update(productionReady=True), lambda r: r.update(manifestSha256='ff'*32),
            lambda r: r['blocks'][0].update(stateRoot=hx(2)), lambda r: r.update(ordinaryRuntimeBlocks=0),
            lambda r: r['blocks'][0].update(forbiddenExtrinsicsRejected=0)]
        for mutation in mutations:
            altered = copy.deepcopy(result); mutation(altered); self.transcript(altered)
            with self.assertRaises(ValueError): identity.validate(altered, context, 'activation', output, pins)

    def test_validate_rejects_snapshot_gaps_mutations_and_rebinding(self):
        result, context, output, pins = self.witness()
        path = output / 'snapshots/000001.json'; original = path.read_bytes()
        path.write_bytes(original + b' ')
        with self.assertRaises(ValueError): identity.validate(result, context, 'activation', output, pins)
        pins[str(path)] = release.digest_file(path)
        with self.assertRaises(ValueError): identity.validate(result, context, 'activation', output, pins)
        path.write_bytes(original); pins[str(path)] = release.digest_file(path)
        (output / 'snapshots/000003.json').write_bytes(b'{}')
        with self.assertRaises(ValueError): identity.validate(result, context, 'activation', output, pins)

    def test_validate_hold_signers_counter_and_destination_are_exact(self):
        result, context, output, pins = self.witness('hold')
        for mutate in [lambda w: w.update(operationCounter=1), lambda w: w.update(destinationSpec=136),
            lambda w: w.update(recoveryAnchor=scale.CHECKPOINT_HASH),
            lambda w: w.update(signers=[]), lambda w: w.update(signers=[hx(1)]),
            lambda w: w.update(signers=w['signers']*2), lambda w: w.update(storedCodeBlake2=hx(2))]:
            altered = copy.deepcopy(result); mutate(altered['suppliedCouncil']['authorizations'][0])
            with self.assertRaises(ValueError): identity.validate(altered, context, 'hold', output, pins)

    def run_mock(self, mutate=None):
        context = self.bind()
        report = {'artifactIdentityReplay': context, 'artifacts': list(self.records.values()),
            'gates': {identity.GATE: {'status': 'unverified'}, 'fullAndWarpSynchronization': {'status': 'unverified'}}, 'productionReady': False}
        binary = self.root / 'synthetic-verifier'; binary.write_bytes(b'never executed'); binary.chmod(0o555)
        paths = dict(self.paths)
        count = len(report['artifacts'])
        def process(command, cwd, output, guard, **kwargs):
            self.assertEqual(command[1:3], ['rehearse-maintenance', '--manifest'])
            self.assertGreaterEqual(kwargs['timeout'], 3600)
            guard(); output.mkdir(parents=True)
            scenario = output.name
            owned = copy.deepcopy(context)
            original_paths = self.paths
            self.paths = {role: Path(cwd)/'inputs'/self.records[role]['sourcePath'] for role in context['inputRoles']}
            try: result, _, _, _ = self.witness(scenario, owned, output / 'replay')
            finally: self.paths = original_paths
            raw = json.dumps(result).encode()
            receipt = {'outputs': {}}
            for name, value in [('launch.json', {'synthetic': True}), ('started.json', {'pid': 1}),
                ('stdout.log', raw), ('stderr.log', b'')]:
                pin = production_build.write(output/name, value); kwargs['retain'](output/name, pin)
                if name in ('stdout.log', 'stderr.log'): receipt['outputs'][name] = pin
            pin = production_build.write(output/'execution.json', receipt); kwargs['retain'](output/'execution.json', pin)
            return raw, receipt
        actual_validate = identity.validate
        def validate(result, bound, scenario, output, pins):
            actual_validate(result, bound, scenario, output, pins)
            if mutate: mutate(result, bound, scenario, output, pins)
        with patch.object(production_build, 'process', side_effect=process), patch.object(identity, 'validate', side_effect=validate):
            try: identity.run(report, paths, binary, identity.sha(binary.read_bytes()), self.root/'runs', release.local_file, release.digest_file)
            except BaseException:
                self.assertEqual(len(report['artifacts']), count)
                self.assertEqual(report['gates'][identity.GATE]['status'], 'unverified')
                self.assertFalse(any(name.startswith('artifactIdentityOutput-') for name in paths))
                raise
        self.assertEqual(report['gates'][identity.GATE]['status'], 'verified')
        self.assertEqual(report['gates']['fullAndWarpSynchronization']['status'], 'unverified')
        self.assertFalse(report['productionReady'])
        return report

    def test_mock_run_clears_only_identity_after_both_fresh_scenarios(self):
        report = self.run_mock()
        self.assertEqual(set(report['gates'][identity.GATE]['scenarios']), {'activation', 'hold'})

    def test_mock_run_rejects_post_validation_file_mutation_without_partial_registration(self):
        def mutate(result, bound, scenario, output, pins):
            if scenario == 'activation':
                with (output/'snapshots/000001.json').open('ab') as stream: stream.write(b' ')
        with self.assertRaises(ValueError): self.run_mock(mutate)

    def test_mock_run_rejects_earlier_scenario_receipt_mutation_during_hold(self):
        def mutate(result, bound, scenario, output, pins):
            if scenario == 'hold':
                with (output.parent.parent/'activation/execution.json').open('ab') as stream: stream.write(b' ')
        with self.assertRaises(ValueError): self.run_mock(mutate)

    def native_witness(self):
        self.dynamic()
        result, context, output, pins = self.witness()
        number = scale.CHECKPOINT + 1; row = result['blocks'][0]
        native = context['scenarios']['activation']['native']
        prefix = f'native-operation-{number}/'
        def emit(name, value):
            path = output / (prefix + name); path.parent.mkdir(parents=True, exist_ok=True)
            raw = json.dumps(value, separators=(',', ':')).encode() if isinstance(value, dict) else value
            path.write_bytes(raw); pins[str(path)] = release.digest_file(path)
            return pins[str(path)]['sha256']
        obj = b'synthetic native snapshot object'
        obj_ref = {'file': 'snapshot-objects/' + identity.sha(obj) + '.scale.gz', 'sha256': identity.sha(obj)}
        emit('recorded/' + obj_ref['file'], obj)
        snapshot_sha = emit('recorded/snapshots/000000.json', {'schema': 'sora-nexus-maintenance-snapshot-v1',
            'index': 0, 'parent': None, 'base': obj_ref, 'delta': None, 'root': row['stateRoot'], 'rawSnapshotSha256': identity.sha(b'raw')})
        authored = (output/f'{number}.block.scale').read_bytes()
        emit('recorded/block.scale', authored)
        emit('recorded/original-extrinsic.scale', scale.vec(identity.block_header(authored)['extrinsics'][1]))
        native_report = {'blockSha256': row['blockSha256'], 'stateRoot': row['stateRoot'],
            'engineSha256': context['wasm']['134']['sha256'], 'policyCommitment': '0x' + context['policyCommitment'],
            'terminalSnapshotSha256': snapshot_sha}
        report_sha = emit('recorded/report.json', native_report)
        receipt = {'schema': 'sora-nexus-native-operation-process-v1', 'pid': 42, 'exitCode': 0, 'success': True,
            'stopReason': None, 'executableSha256': native['executableSha256'], 'executable': context['inputPaths'][native['executableRole']],
            'sourceFinalityVerified': False, 'productionReady': False,
            'stdoutSha256': emit('stdout.json', native_report), 'stderrSha256': emit('stderr.log', b''),
            'manifestSha256': emit('manifest.json', {'synthetic': True})}
        derived = {'block': number, 'operationIndex': 0, 'executableSha256': native['executableSha256'],
            'engineBindingSha256': native['bindingSha256'],
            'parentReplayTranscriptSha256': identity.sha(b'sora-nexus-maintenance-transcript-v1' + result['manifestSha256'].encode() + result['captureManifestSha256'].encode()),
            'reportFile': prefix + 'recorded/report.json', 'reportSha256': report_sha,
            'processReceiptFile': prefix + 'process.json', 'processReceiptSha256': emit('process.json', receipt)}
        for name in ['joinedToVerifiedReplayPrefix', 'exactAuthoredBlockBytesMatch', 'completeOperationDeltaCompared', 'nativeTerminalSnapshotIndependentlyRestored']:
            derived[name] = True
        for name in ['nativeSourceBuildAuthenticityVerified', 'sourceFinalityVerified', 'economicPolicyIndependentlyVerified', 'fixtureOnly', 'productionReady']:
            derived[name] = False
        result['nativeOperationDerivations'] = [derived]; result['nativeExecution'] = True
        return result, context, output, pins

    def test_validate_native_child_snapshot_closure_cannot_be_rebound(self):
        result, context, output, pins = self.native_witness()
        identity.validate(result, context, 'activation', output, pins)
        path = output/f'native-operation-{scale.CHECKPOINT+1}/recorded/snapshots/000000.json'
        path.write_bytes(path.read_bytes() + b' ')
        pins[str(path)] = release.digest_file(path)
        with self.assertRaises(ValueError): identity.validate(result, context, 'activation', output, pins)

    def test_validate_native_exact_extrinsic_and_transcript_required(self):
        result, context, output, pins = self.native_witness()
        path = output/f'native-operation-{scale.CHECKPOINT+1}/recorded/original-extrinsic.scale'
        raw = path.read_bytes(); path.write_bytes(raw + b'\0'); pins[str(path)] = release.digest_file(path)
        with self.assertRaises(ValueError): identity.validate(result, context, 'activation', output, pins)
        path.write_bytes(raw); pins[str(path)] = release.digest_file(path)
        result['nativeOperationDerivations'][0]['parentReplayTranscriptSha256'] = 'ff'*32
        with self.assertRaises(ValueError): identity.validate(result, context, 'activation', output, pins)

    def test_mock_run_rejects_stdout_mutation_after_validation(self):
        def mutate(result, bound, scenario, output, pins):
            if scenario == 'hold':
                with (output.parent/'stdout.log').open('ab') as stream: stream.write(b' ')
        with self.assertRaises(ValueError): self.run_mock(mutate)

    def test_mock_run_rejects_mutation_during_final_registration(self):
        original = production_build.digest
        changed = False
        def digest(path):
            nonlocal changed
            result = original(path)
            if Path(path).name == 'verification.json' and not changed:
                changed = True
                target = Path(path).parent/'activation/execution.json'
                with target.open('ab') as stream: stream.write(b' ')
            return result
        with patch.object(production_build, 'digest', side_effect=digest):
            with self.assertRaises(ValueError): self.run_mock()
        self.assertTrue(changed)


if __name__ == '__main__':
    unittest.main()
