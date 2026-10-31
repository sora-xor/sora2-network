"""Synthetic joins and mocked processes only; no runtime, signature or Cargo execution."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import artifact_identity_release as identity
import forward_repair_release as repair
import incoming_replay_release as incoming
import production_build
import release_package as release
import hashi_codec as scale
import repair_staging_release as staging
from test_artifact_identity_release import ArtifactIdentityTests as Fixtures
from test_activation_post_state_release import activation_witness
from test_activation_block_weight import METADATA
from test_release_package import hx, policy, wasm


class ForwardRepairTests(unittest.TestCase):
    setUp = Fixtures.setUp
    put, ref, step, save_manifests = Fixtures.put, Fixtures.ref, Fixtures.step, Fixtures.save_manifests
    transcript = Fixtures.transcript

    def auth_bytes(self, destination, version, phase, counter, cursor, active):
        original = identity.blake(self.paths['policyScale'].read_bytes())
        stored = self.records['checkpointCode' if version == 134 else 'holdWasm']['blake2_256']
        executing = self.records['bootstrapWasm' if version == 134 else 'holdWasm']['blake2_256']
        target = self.records['holdWasm' if destination == 135 else 'repairWasm']['blake2_256']
        raw = bytes.fromhex(original) + bytes([['Applying', 'Auditing', 'Ready', 'Hold', 'Complete', 'Failed'].index(phase)])
        raw += scale.u32(cursor) + counter.to_bytes(8, 'little') + bytes.fromhex(stored + executing + target)
        raw += scale.u32(destination) + (bytes(32) if destination == 135 else bytes.fromhex(active['commitment']))
        raw += release.authorization_domain_scale(release.policy_identity(self.paths['policyScale'].read_bytes())['authorizationDomain'])
        lineage = scale.MAGIC + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + bytes.fromhex(original)
        logs = [b'\x06BABE' + scale.vec(b'\x02' + scale.u32(0) + (101).to_bytes(8, 'little')), b'\x00' + scale.vec(lineage)]
        first_header = (bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + scale.compact(scale.CHECKPOINT + 1)
                        + bytes.fromhex(hx(90)[2:]) + bytes(32) + scale.compact(len(logs)) + b''.join(logs))
        raw += bytes.fromhex(identity.blake(first_header))
        council = identity.policy_controls(self.paths['policyScale'].read_bytes())[1]
        signatures = scale.compact(len(council)) + b''.join(bytes.fromhex(key) + b'\x01' + b'\x77' * 64 for key in council)
        envelope = b'snxrec01\x01\x04' + raw + signatures + (b'\0' if destination == 135 else b'\1' + self.paths['incomingActivePolicyScale'].read_bytes())
        return raw, envelope

    def stage_repair(self, destination=136):
        active = release.policy_identity(self.paths['incomingActivePolicyScale'].read_bytes())
        raw, _ = self.auth_bytes(destination, 135, 'Hold', 1, 1, active)
        code = self.paths['repairWasm'].read_bytes()
        chunks = [code[offset:offset + staging.CHUNK] for offset in range(0, len(code), staging.CHUNK)]
        manifest = raw + (0).to_bytes(8, 'little') + scale.u32(len(code))
        manifest += scale.compact(len(chunks)) + b''.join(bytes.fromhex(identity.blake(chunk)) for chunk in chunks)
        council = identity.policy_controls(self.paths['policyScale'].read_bytes())[1]
        signatures = scale.compact(len(council)) + b''.join(bytes.fromhex(key) + b'\x01' + b'\x66' * 64 for key in council)
        self.put('repairStageAuthorization', b'snxrec01\x01\x0c' + manifest + signatures)
        self.assertEqual(len(chunks), 1)
        self.put('repairStageCode', b'snxrec01\x01\x03\x02' + scale.u32(0) + scale.vec(chunks[0]))

    def prepare(self, maintenance_gap=14_400):
        original_raw = self.paths['policyScale'].read_bytes()
        active_raw = original_raw[:167] + bytes([44]) * 32 + original_raw[199:]
        self.put('incomingActivePolicyScale', active_raw)
        self.put('repair136PolicyScale', active_raw)
        self.put('repairWasm', wasm(136, 132))
        original, active = release.policy_identity(original_raw), release.policy_identity(active_raw)
        base = {'operatorsScale': '0x' + self.paths['operatorsScale'].read_bytes().hex(), 'previousGrandpaSetId': 44,
            'firstRecoverySlot': 101, 'finalWasm': self.paths['finalWasm'].name}
        self.put('holdBinding', base)
        self.put('repair136Binding', {**base, 'policyScale': '0x' + active_raw.hex(), 'policyCommitment': '0x' + active['commitment'],
            'originalPolicyScale': '0x' + original_raw.hex(), 'originalPolicyCommitment': '0x' + original['commitment']})
        pair = lambda role: {key: self.records[role][key] for key in ('sha256', 'blake2_256')}
        self.put('repair136Provenance', {'formatVersion': 1, 'deployedSourceTree': release.LEGACY_TREE, 'sdk': 'polkadot-stable2603-3',
            'specVersion': 136, 'transactionVersion': 132, 'bound': True, 'releaseReady': False,
            'bindingEvidence': {'binding': pair('repair136Binding'), 'policy': pair('repair136PolicyScale'),
                'operators': pair('operatorsScale'), 'finalWasm': pair('finalWasm'), 'holdWasm': None}})
        for name in ('cargo.stdout.log', 'cargo.stderr.log'): self.put(name, b'nonexecuted synthetic log')
        self.put('repair136BuildReceipt', {'status': 'passed', 'exitCode': 0, 'specVersion': 136, 'transactionVersion': 132,
            'sourceProvenance': release.digest_file(self.paths['repair136Provenance']), 'productionReady': False,
            'artifacts': {'runtime.wasm': release.digest_file(self.paths['repairWasm'])},
            'logs': {name: release.digest_file(self.paths[name]) for name in ('cargo.stdout.log', 'cargo.stderr.log')}})
        for role in ('incomingDeliveryInventory', 'incomingReconciliation', 'externalFinalizedHeads', 'recoveryWorkIndex'):
            self.put(role, {'syntheticOnly': True})
        configs = []
        for version, wasm_role, policy_role in [(134, 'bootstrapWasm', 'policyScale'), (136, 'repairWasm', 'incomingActivePolicyScale')]:
            prefix = 'native' + str(version)
            self.put(prefix + 'Executable', b'synthetic nonexecuted native' + bytes([version]), executable=True)
            binding = {'schema': 'sora-nexus-native-operation-engine-binding-v' + ('1' if version == 134 else '2'),
                'engineSha256': self.records[wasm_role]['sha256'], 'engineBlake2': self.records[wasm_role]['blake2_256'],
                'policySha256': self.records[policy_role]['sha256'], 'policyCommitment': identity.blake(self.paths[policy_role].read_bytes()),
                'sdk': 'polkadot-stable2603-3', 'specVersion': version, 'transactionVersion': 131 if version == 134 else 132,
                'fixtureOnly': False, 'productionReady': False, 'runtimeFeatureProfile': {}, 'hostLockAmendment': {}, 'sdkCompilationRepair': {}}
            if version == 136:
                binding.update(originalPolicySha256=self.records['policyScale']['sha256'], originalPolicyCommitment=original['commitment'], templateCarriesReviewedSdkAndTestPathRepairs=True)
            self.put(prefix + 'Binding', binding)
            configs.append({'executable': self.ref(prefix + 'Executable'), 'engineBinding': self.ref(prefix + 'Binding')})
        for role, tag in [('originalIncoming', 6), ('repairedIncoming', 6), ('historicalAudit', 9)]:
            self.put(role, b'snxrec01\x01' + bytes([tag]) + scale.vec(scale.u32(0) + b'synthetic operation'))
        for role, destination, version, phase, counter in [('holdEscape', 135, 134, 'Applying', 0), ('repairEscape', 136, 135, 'Hold', 1)]:
            _, envelope = self.auth_bytes(destination, version, phase, counter, 1, active)
            self.put(role, envelope)
        self.stage_repair()
        self.repair_manifest = {**{key: copy.deepcopy(value) for key, value in self.hold.items() if key not in ('steps', 'suppliedCouncil')},
            'repairs': [self.ref('repairWasm')], 'nativeOperationRecorder': configs[0], 'nativeOperationRecordersV2': configs[1:],
            'steps': [self.step(101, 'originalIncoming', 134, 'Applying'), self.step(102, 'holdEscape', 135, 'Hold'),
                self.step(103, 'repairStageAuthorization', 135, 'Hold'), self.step(104, 'repairStageCode', 135, 'Hold'),
                self.step(105, 'repairEscape', 136, 'Applying'), self.step(106, 'repairedIncoming', 136, 'Applying'),
                self.step(107, 'historicalAudit', 136, 'Auditing'), self.step(108, 'terminal', 138, 'Complete'),
                self.step(109, None, 138, 'Complete'), self.step(110, None, 138, 'Complete'), self.step(111, None, 138, 'Complete')],
            'suppliedCouncil': {'escapes': [{'step': 1, 'destinationSpec': 135, 'envelopeSha256': self.records['holdEscape']['sha256']},
                {'step': 4, 'destinationSpec': 136, 'envelopeSha256': self.records['repairEscape']['sha256']}],
                'expectedTerminal': {'specVersion': 138, 'phase': 'Complete', 'cursor': active['workChunks'], 'auditCursor': active['auditChunks'],
                    'operationCounter': 2, 'activePolicyCommitment': '0x' + active['commitment'], 'stateRoot': hx(100), 'ordinaryBlocks': 3,
                    'terminalAuditBlake2': '0x' + identity.blake(bytes.fromhex(active['commitment']) + active_raw[-32:] + scale.u32(active['workChunks']))}}}
        self.schedule_repair(maintenance_gap)
        self.save_repair()
        return self.bind_repair()

    def schedule_repair(self, maintenance_gap=14_400):
        # Keep the signed first-child anchor unchanged. Like the real fixture,
        # exercise the long pause in a later maintenance child.
        for index, step in enumerate(self.repair_manifest['steps']):
            step['slot'] = 101 + index + (maintenance_gap - 1 if index else 0)
            step['timestamp'] = step['slot'] * 6000
        normal = self.repair_manifest['steps'][-3:]
        boundary = (normal[0]['slot'] // 600 + 1) * 600
        for offset, step in enumerate(normal[1:]):
            step['slot'] = boundary + offset
            step['timestamp'] = step['slot'] * 6000

    def save_repair(self):
        self.put('recoveryParentReplay', self.repair_manifest, 'repair-replay.json')
        self.paths[repair.ROLE] = self.paths['recoveryParentReplay']
        self.records[repair.ROLE] = {**self.records['recoveryParentReplay'], 'role': repair.ROLE}

    def bind_repair(self):
        original = release.policy_identity(self.paths['policyScale'].read_bytes())
        identity_context = identity.bind(self.root, self.records, self.paths, release.local_file, release.digest_file)
        incoming_context = incoming.bind(self.root, self.records, self.paths, original, release.local_file, release.digest_file, release.policy_identity, release.wasm_identity)
        return repair.bind(self.root, self.records, self.paths, original, identity_context, incoming_context, release.local_file, release.digest_file)

    def test_binds_exact_branch_and_requires_all_executing_versions(self):
        context = self.prepare()
        self.assertEqual({row['executingSpec'] for row in context['scenarios']['repair']['steps']}, {134, 135, 136, 138})
        self.assertEqual(context['repairs'], [136])
        self.assertFalse(context['productionReady'])

    def test_rejects_missing_identity_incoming_or_alias_pair(self):
        self.prepare()
        for role in ('artifactHoldReplay', 'incomingActivePolicyScale', 'incomingDeliveryInventory'):
            path = self.paths.pop(role)
            with self.assertRaises((ValueError, KeyError)): self.bind_repair()
            self.paths[role] = path
        self.paths[repair.ROLE] = self.paths['artifactHoldReplay']
        with self.assertRaises(ValueError): self.bind_repair()

    def test_rejects_install_without_execution_unknown_fields_and_different_controls(self):
        self.prepare(); original = copy.deepcopy(self.repair_manifest)
        changes = [lambda m: m['steps'].pop(), lambda m: m.update(publicDynamicRepair=None),
            lambda m: m.update(previousGrandpaSet=45), lambda m: m['steps'][0].update(slot=100),
            lambda m: m['steps'][3].update(expectedSpec=137), lambda m: m['suppliedCouncil']['expectedTerminal'].update(operationCounter=3)]
        for change in changes:
            self.repair_manifest = copy.deepcopy(original); change(self.repair_manifest); self.save_repair()
            with self.assertRaises((ValueError, KeyError)): self.bind_repair()

    def test_native_engine_active_original_policy_and_fixture_bindings_are_exact(self):
        self.prepare(); original = json.loads(self.paths['native136Binding'].read_text())
        for key, value in [('engineSha256', 'ff' * 32), ('originalPolicyCommitment', 'ff' * 32), ('policyCommitment', 'ff' * 32),
            ('fixtureOnly', True), ('specVersion', 137), ('transactionVersion', 134)]:
            changed = {**original, key: value}; self.put('native136Binding', changed)
            self.repair_manifest['nativeOperationRecordersV2'][0]['engineBinding'] = self.ref('native136Binding'); self.save_repair()
            with self.assertRaises(ValueError): self.bind_repair()

    def witness(self, context=None, output=None):
        context = context or self.prepare()
        replay, context, output, pins = Fixtures.witness(self, 'repair', context, output)
        plan = context['scenarios']['repair']; rows = replay['blocks']
        def emit(name, value):
            path = output / name; path.parent.mkdir(parents=True, exist_ok=True)
            raw = json.dumps(value, separators=(',', ':')).encode() if isinstance(value, dict) else value
            path.write_bytes(raw); pins[str(path)] = release.digest_file(path)
            return pins[str(path)]['sha256']
        for index, (row, detail) in enumerate(zip(rows, plan['steps'])):
            row['cursor'] = 0 if (detail['action'] == 4 and detail['step']['expectedSpec'] in (136, 137)) or (index and rows[index - 1]['cursor'] == 0 and detail['action'] == 0) else 1
            row['operationCounter'] = detail['counterAfter']
            row['forbiddenExtrinsicsRejected'] = 0 if detail['executingSpec'] == 138 else 2 + detail['counterBefore']
        replay.update(automaticActivationObserved=True, councilHoldObserved=True, forwardRepairObserved=True,
            ordinaryRuntimeBlocks=3, feeClockShiftObserved=True, nativeExecution=True)
        replay['activationValidation'] = activation_witness(rows, plan['steps'],
            context['policies']['138']['commitment'], context['wasm']['138']['blake2'])
        replay['activationValidation']['lineagePolicyCommitment'] = context['policyCommitment']
        emit('activation-metadata.scale', METADATA)
        self.transcript(replay)
        witnesses = []
        for escape in plan['escapes']:
            index = escape['step']; detail = plan['steps'][index]; prior = rows[index - 1]
            destination, version = detail['step']['expectedSpec'], detail['executingSpec']
            raw, envelope = self.auth_bytes(destination, version, prior['phase'], detail['counterBefore'], prior['cursor'], context['policies']['136'])
            number = scale.CHECKPOINT + index + 1; prefix = f'{number}.supplied.'
            message = scale.vec(b'sora-nexus/recovery-forward-upgrade/v2') + bytes.fromhex(scale.GENESIS_HASH[2:]) + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + raw
            witness = {'step': index, 'blockNumber': number, 'parentHash': prior['hash'],
                'envelopeSha256': emit(prefix + 'escape.scale', envelope), 'authorizationSha256': emit(prefix + 'authorization.scale', raw),
                'signingMessageSha256': emit(prefix + 'signing-message.scale', message), 'phase': prior['phase'], 'cursor': prior['cursor'],
                'operationCounter': detail['counterBefore'], 'storedCodeBlake2': '0x' + (context['checkpointCodeBlake2'] if version == 134 else context['wasm'][str(version)]['blake2']),
                'executingCodeBlake2': '0x' + context['wasm'][str(version)]['blake2'], 'destinationCodeBlake2': '0x' + context['wasm'][str(destination)]['blake2'],
                'destinationSpec': destination, 'revisedPolicyCommitment': hx(0) if destination == 135 else '0x' + context['policies']['136']['commitment'],
                'recoveryAnchor': rows[0]['hash'],
                'signers': ['0x' + key for key in context['council']]}
            emit(prefix + 'authorization.json', witness); witnesses.append(witness)
        replay['suppliedCouncil'] = {'schema': 'sora-nexus-supplied-council-replay-witness-v1', 'authorizations': witnesses,
            'terminal': plan['manifest']['suppliedCouncil']['expectedTerminal'], 'signaturesVerifiedAgainstPinnedPolicy': True,
            'automaticActivationObserved': True, **{key: False for key in ('publicTestCouncilGenerated', 'councilInventoryApprovalVerified', 'sourceProvenanceVerified', 'productionReady', 'publicDynamicFixture')}}
        replay['repairStagingAuthorizations'] = []
        for item in plan['repairStaging']:
            index = item['step']; row = rows[index]; prefix = f'{row["number"]}.repair-staging.'
            raw = bytes.fromhex(item['manifestScale'])
            witness = {key: value for key, value in item.items() if key in staging.FIELDS}
            witness.update(schema='sora-nexus-executed-repair-staging-authorization-v1',
                step=index, blockNumber=row['number'], blockHash=row['hash'], stateRoot=row['stateRoot'],
                diskSnapshotManifestSha256=row['diskSnapshotManifestSha256'],
                quorumVerified=True, productionReady=False, independentImportAndRestartVerified=True,
                signingMessageSha256=emit(prefix + 'signing-message.scale', staging.message(raw)))
            emit(prefix + 'manifest.scale', raw)
            emit(prefix + 'envelope.scale', self.paths[item['recoveryRole']].read_bytes())
            emit(prefix + 'authorization.json', witness)
            replay['repairStagingAuthorizations'].append(witness)
        transcript = identity.sha(b'sora-nexus-maintenance-transcript-v1' + replay['manifestSha256'].encode() + replay['captureManifestSha256'].encode())
        for index, row in enumerate(rows):
            if index in plan['dynamicSteps']:
                detail = plan['steps'][index]; version = str(detail['executingSpec']); native = plan['native'][version]
                number = row['number']; prefix = f'native-operation-{number}/'; policy_data = detail['policy']
                obj = b'synthetic native object'; objref = {'file': 'snapshot-objects/' + identity.sha(obj) + '.scale.gz', 'sha256': identity.sha(obj)}
                emit(prefix + 'recorded/' + objref['file'], obj)
                snapshot = emit(prefix + 'recorded/snapshots/000000.json', {'schema': 'sora-nexus-maintenance-snapshot-v1', 'index': 0,
                    'parent': None, 'base': objref, 'delta': None, 'root': row['stateRoot'], 'rawSnapshotSha256': identity.sha(b'synthetic raw')})
                raw_block = (output / f'{number}.block.scale').read_bytes()
                emit(prefix + 'recorded/block.scale', raw_block)
                emit(prefix + 'recorded/original-extrinsic.scale', scale.vec(identity.block_header(raw_block)['extrinsics'][1]))
                native_report = {'schema': 'sora-nexus-native-operation-expectations-v' + ('1' if version == '134' else '2'),
                    'engineSha256': context['wasm'][version]['sha256'], 'policyCommitment': '0x' + policy_data['commitment'],
                    'blockSha256': row['blockSha256'], 'stateRoot': row['stateRoot'], 'terminalSnapshotSha256': snapshot,
                    'engineBinding': self.paths[native['bindingRole']].read_text(),
                    **{key: True for key in ('nativeWasmOperationParityVerified', 'independentImportVerified', 'freshDiskRestoreVerified', 'completeTopWriteSetCompared')},
                    **{key: False for key in ('fixtureOnly', 'productionReady', 'consensusPrefixWaiver', 'compiledPolicyOverrideUsed', 'configurationInjected')}}
                if version != '134':
                    pending_handover = index == witnesses[-1]['step'] + 1
                    accepted = [{key: witness[key] for key in repair.AUTH_FIELDS} for witness in witnesses if witness['step'] < index]
                    parent = rows[index - 1]
                    work_context = {'schema': 'sora-nexus-native-verified-work-context-v2', 'parentHash': parent['hash'],
                        'parentStateRoot': parent['stateRoot'], 'parentSnapshotManifestSha256': parent['diskSnapshotManifestSha256'],
                        'priorTranscriptSha256': transcript, 'operationCounter': detail['counterBefore'],
                        'acceptedEscapesSha256': identity.sha(json.dumps(accepted, separators=(',', ':')).encode()),
                        'parentRepairAuthorization': '0x' + (output / f'{witnesses[-1]["blockNumber"]}.supplied.authorization.scale').read_bytes().hex() if pending_handover else None}
                    native_report.update(originalPolicyCommitment='0x' + context['policyCommitment'], originalPolicySha256=context['artifactBindings']['policyScale'],
                        activePolicySha256=context['artifactBindings'][policy_data['role']], specVersion=int(version), transactionVersion=132,
                        operationCounter=detail['counterBefore'], parentRepairAuthorizationSha256=witnesses[-1]['authorizationSha256'] if pending_handover else None,
                        repairHandoverLifecycleChecked=True, recoveryContextSha256=emit(prefix + 'recovery-context.json', work_context))
                report_sha = emit(prefix + 'recorded/report.json', native_report)
                receipt = {'schema': 'sora-nexus-native-operation-process-v1', 'pid': 42, 'exitCode': 0, 'success': True,
                    'stopReason': None, 'executable': context['inputPaths'][native['executableRole']], 'executableSha256': native['executableSha256'],
                    'sourceFinalityVerified': False, 'productionReady': False, 'stdoutSha256': emit(prefix + 'stdout.json', native_report),
                    'stderrSha256': emit(prefix + 'stderr.log', b''), 'manifestSha256': emit(prefix + 'manifest.json', {'syntheticOnly': True})}
                derivation = {'block': number, 'operationIndex': detail['operationIndex'], 'executableSha256': native['executableSha256'],
                    'engineBindingSha256': native['bindingSha256'], 'parentReplayTranscriptSha256': transcript,
                    'reportFile': prefix + 'recorded/report.json', 'reportSha256': report_sha,
                    'processReceiptFile': prefix + 'process.json', 'processReceiptSha256': emit(prefix + 'process.json', receipt),
                    **{key: True for key in ('joinedToVerifiedReplayPrefix', 'exactAuthoredBlockBytesMatch', 'completeOperationDeltaCompared', 'nativeTerminalSnapshotIndependentlyRestored')},
                    **{key: False for key in ('nativeSourceBuildAuthenticityVerified', 'sourceFinalityVerified', 'economicPolicyIndependentlyVerified', 'fixtureOnly', 'productionReady')}}
                replay['nativeOperationDerivations' if version == '134' else 'repairedNativeOperationDerivations'].append(derivation)
            transcript = identity.sha(transcript.encode() + json.dumps({key: row[key] for key in identity.BLOCK_FIELDS}, separators=(',', ':')).encode())
        flags = {key: False for key in ('publicTestCouncilFixture', 'externalFinalityVerified', 'operatorApprovalVerified', 'consensusSealsVerified', 'productionReady')}
        prior = {'block': rows[0], 'executingCodeBlake2': rows[0]['executingCodeBlake2'], 'originalPolicy': context['policyCommitment'],
            'activePolicy': context['policyCommitment'], 'nativeComparison': replay['nativeOperationDerivations'][0], **flags}
        active = context['policies']['136']['commitment']
        audit_index = next(index for index, detail in enumerate(plan['steps']) if detail['action'] == 9)
        proof = {'block': rows[audit_index], 'executingCodeBlake2': rows[audit_index]['executingCodeBlake2'], 'originalPolicy': context['policyCommitment'],
            'activePolicy': active, 'consumingBlock': prior, 'consumedPolicy': prior['activePolicy'], 'auditCommitment': 'aa' * 32, **flags,
            'finalValidation': {'block': rows[-1], 'activePolicy': active, 'auditedPolicy': active, 'auditedBlock': rows[audit_index]['number'], 'auditedCounter': 2,
                'consumptionBlock': rows[0]['number'], 'consumptionCounter': 0, 'readOnlyStateRootMatches': True, 'pendingIncomingAudits': 0, 'productionReady': False}}
        replay['priorPolicyIncomingAudits'] = [proof]
        certificate = {'checkpointStateRoot': identity.history.ROOT, 'captureManifestSha256': context['captureManifestSha256'],
            'replayManifestSha256': plan['manifestSha256'], 'replayTranscriptSha256': replay['replayTranscriptSha256'], 'chainLocalReplayVerified': True,
            'finalPolicyCommitment': active, 'finalStateRoot': rows[-1]['stateRoot'], **flags}
        replay['incomingReplayBinding'] = copy.deepcopy(certificate)
        result = {'schema': 'sora-nexus-incoming-coverage-verification-v1', 'checkpointHash': scale.CHECKPOINT_HASH,
            'checkpointStateRoot': identity.history.ROOT, 'policyCommitment': '0x' + active, 'priorPolicyIncomingOperations': 1,
            'priorPolicyReplay': certificate, 'recoveryReplay': replay, 'evidenceHashes': [],
            'checkpointMetadata': {'schema': 'sora-nexus-checkpoint-metadata-witness-v1', 'checkpointHash': scale.CHECKPOINT_HASH,
                'stateRoot': identity.history.ROOT, 'wasmExecutionVerified': True, 'nativeExecution': False},
            **{key: True for key in ('localPendingCoverageVerified', 'declaredExternalInventoryCoverageVerified', 'workCoverageVerified')},
            **{key: False for key in ('externalArchiveCompletenessVerified', 'externalFinalityVerified', 'authenticationVerified', 'exactArtifactExecutionVerified', 'productionReady')}}
        for field, role in [('captureManifestSha256', 'checkpointCaptureManifest'), ('policySha256', 'incomingActivePolicyScale'),
            ('incomingWorkSha256', 'recoveryWorkIndex'), ('incomingInventorySha256', 'incomingDeliveryInventory'),
            ('incomingReconciliationSha256', 'incomingReconciliation'), ('externalFinalizedHeadsSha256', 'externalFinalizedHeads')]:
            result[field] = context['artifactBindings'][role]
        return result, context, output, pins

    def test_validates_complete_synthetic_execution_auth_native_and_audit_closure(self):
        result, context, output, pins = self.witness()
        repair.validate(result, context, 'repair', output, pins)

    def test_repair_cannot_skip_final_activation_observer(self):
        result, context, output, pins = self.witness()
        del result['recoveryReplay']['activationValidation']
        with self.assertRaisesRegex(ValueError, 'repaired final runtime activation gate failed'):
            repair.validate(result, context, 'repair', output, pins)

    def test_repair_rejects_detached_final_activation_policy(self):
        result, context, output, pins = self.witness()
        result['recoveryReplay']['activationValidation']['policyCommitment'] = context['policyCommitment']
        with self.assertRaisesRegex(ValueError, 'activation policy or exact runtime differs'):
            repair.validate(result, context, 'repair', output, pins)

    def test_repaired_138_rejects_short_maintenance_with_complete_boundary_observations(self):
        result, context, output, pins = self.witness(self.prepare(maintenance_gap=14_399))
        maintenance = result['recoveryReplay']['activationValidation']['maintenance']
        self.assertEqual(maintenance['maximumSlotGap'], 14_399)
        self.assertFalse(maintenance['coverage']['complete'])
        with self.assertRaisesRegex(ValueError, 'repaired final runtime activation gate failed'):
            repair.validate(result, context, 'repair', output, pins)

    def test_repaired_138_requires_session_era_and_followup_execution(self):
        result, context, output, pins = self.witness()
        for transition_in_last_block in (False, True):
            changed = copy.deepcopy(result)
            witness = changed['recoveryReplay']['activationValidation']
            initial = copy.deepcopy(witness['rows'][0])
            for row in witness['rows']:
                for key in ('epoch', 'session', 'currentEra', 'activeEra', 'activeEraStart',
                            'asyncState', 'lastValidatorEra'):
                    row[key] = copy.deepcopy(initial[key])
            witness['transitions'] = {'firstSession': None, 'firstActiveEra': None,
                'postFirstSessionBlocks': 0, 'postFirstActiveEraBlocks': 0}
            if transition_in_last_block:
                last = witness['rows'][-1]
                last.update(epoch=initial['epoch'] + 1, session=101, activeEra=7, activeEraStart=last['timestamp'])
                last['asyncState'].update(queuedSession=None, queuedEra=None)
                last['lastValidatorEra']['queued'] = []
                witness['transitions'].update(
                    firstSession={'block': last['block'], 'before': 100, 'after': 101},
                    firstActiveEra={'block': last['block'], 'before': 6, 'after': 7})
            with self.subTest(transition_in_last_block=transition_in_last_block), self.assertRaisesRegex(
                    ValueError, 'repaired final runtime activation gate failed'):
                repair.validate(changed, context, 'repair', output, pins)

    def test_report_code_counter_policy_scope_and_historical_audit_mutations_rejected(self):
        result, context, output, pins = self.witness()
        changes = [lambda r: r['recoveryReplay']['blocks'][4].update(executingCodeBlake2=context['wasm']['134']['blake2']),
            lambda r: r['recoveryReplay']['blocks'][4].update(operationCounter=1),
            lambda r: r['recoveryReplay'].update(productionReady=True),
            lambda r: r['recoveryReplay']['suppliedCouncil']['authorizations'][1].update(revisedPolicyCommitment=hx(1)),
            lambda r: r['recoveryReplay']['priorPolicyIncomingAudits'][0]['finalValidation'].update(pendingIncomingAudits=1),
            lambda r: r['recoveryReplay']['priorPolicyIncomingAudits'][0].update(consumedPolicy='bb' * 32),
            lambda r: r['recoveryReplay'].update(repairedNativeOperationDerivations=[]),
            lambda r: r.update(priorPolicyReplay=None)]
        for change in changes:
            changed = copy.deepcopy(result); change(changed)
            with self.assertRaises(ValueError): repair.validate(changed, context, 'repair', output, pins)

    def test_raw_authorization_message_snapshot_or_context_mutation_is_rejected(self):
        result, context, output, pins = self.witness()
        names = [f'{scale.CHECKPOINT + 5}.supplied.authorization.scale', f'{scale.CHECKPOINT + 5}.supplied.signing-message.scale',
            'snapshots/000005.json', f'native-operation-{scale.CHECKPOINT + 6}/recorded/snapshots/000000.json',
            f'native-operation-{scale.CHECKPOINT + 6}/recovery-context.json']
        for name in names:
            path = output / name; raw = path.read_bytes(); old = pins[str(path)]
            path.write_bytes(raw + b' '); pins[str(path)] = release.digest_file(path)
            with self.assertRaises(ValueError): repair.validate(result, context, 'repair', output, pins)
            path.write_bytes(raw); pins[str(path)] = old

    def test_raw_staging_outputs_and_native_acceptance_cannot_be_replaced_by_flags(self):
        result, context, output, pins = self.witness()
        baseline = result['recoveryReplay']['repairStagingAuthorizations'][0]
        for key, value in [('step', 3), ('cursor', 0), ('phase', 'Applying'), ('stagingNonceBefore', 1),
            ('stagingNonceAfter', 2), ('blockHash', hx(7)), ('stateRoot', hx(7)), ('recoveryAnchor', hx(7)),
            ('diskSnapshotManifestSha256', '77' * 32), ('quorumVerified', False),
            ('productionReady', True), ('independentImportAndRestartVerified', False),
            ('signers', []), ('policyCommitment', hx(7)), ('destinationCodeBlake2', hx(7))]:
            with self.subTest(field=key):
                changed = copy.deepcopy(result)
                changed['recoveryReplay']['repairStagingAuthorizations'][0][key] = value
                with self.assertRaises(ValueError): repair.validate(changed, context, 'repair', output, pins)
        for missing in (None, []):
            changed = copy.deepcopy(result)
            if missing is None: del changed['recoveryReplay']['repairStagingAuthorizations']
            else: changed['recoveryReplay']['repairStagingAuthorizations'] = missing
            with self.assertRaises(ValueError): repair.validate(changed, context, 'repair', output, pins)
        for suffix in ('manifest.scale', 'signing-message.scale', 'envelope.scale', 'authorization.json'):
            path = output / f'{baseline["blockNumber"]}.repair-staging.{suffix}'
            raw, pin = path.read_bytes(), pins[str(path)]
            path.write_bytes(raw + b'!'); pins[str(path)] = release.digest_file(path)
            with self.assertRaises(ValueError): repair.validate(result, context, 'repair', output, pins)
            path.write_bytes(raw); pins[str(path)] = pin

    def test_missing_staging_or_wrong_raw_code_is_rejected_before_native_launch(self):
        self.prepare(); original = copy.deepcopy(self.repair_manifest)
        for index in (2, 3):
            self.repair_manifest = copy.deepcopy(original)
            self.repair_manifest['steps'][index]['recovery'] = self.ref('tick')
            self.save_repair()
            with self.assertRaisesRegex(ValueError, 'repair'):
                self.bind_repair()
        self.repair_manifest = copy.deepcopy(original)
        raw = self.paths['repairStageCode'].read_bytes()
        self.put('repairStageCode', raw[:-1] + bytes([raw[-1] ^ 1]))
        self.repair_manifest['steps'][3]['recovery'] = self.ref('repairStageCode')
        self.save_repair()
        with self.assertRaisesRegex(ValueError, 'staged bytes'):
            self.bind_repair()

    def test_native_handover_is_null_after_first_repaired_child(self):
        self.prepare()
        self.repair_manifest['steps'].insert(5, self.step(106, 'tick', 136, 'Applying'))
        self.schedule_repair()
        self.repair_manifest['suppliedCouncil']['expectedTerminal']['stateRoot'] = hx(101)
        self.save_repair()
        result, context, output, pins = self.witness(self.bind_repair())
        native = result['recoveryReplay']['repairedNativeOperationDerivations'][0]
        native_report = json.loads((output / native['reportFile']).read_text())
        self.assertIsNone(native_report['parentRepairAuthorizationSha256'])
        repair.validate(result, context, 'repair', output, pins)

    def run_mock(self, mutate=None):
        context = self.prepare()
        binary = self.root / 'nonexecuted-verifier'; binary.write_bytes(b'synthetic never executed'); binary.chmod(0o555)
        binary_sha = identity.sha(binary.read_bytes())
        report = {'forwardRepairReplay': context, 'artifacts': list(self.records.values()),
            'gates': {identity.GATE: {'status': 'verified', 'verifierSha256': binary_sha}, repair.GATE: {'status': 'unverified'},
                'maintenanceEpochLivenessAndStaking': {'status': 'unverified'}, 'incomingPendingCoverage': {'status': 'unverified'}}, 'productionReady': False}
        before = copy.deepcopy(report['gates']); count = len(report['artifacts']); paths = dict(self.paths)
        def process(command, cwd, output, guard, **kwargs):
            self.assertEqual(command[1], 'verify-incoming-coverage')
            self.assertEqual(command[command.index('--recovery-output') + 1], str(output / 'replay'))
            self.assertEqual(command[command.index('--recovery-replay') + 1], str(cwd / 'inputs/repair-replay.json'))
            self.assertEqual(kwargs['timeout'], 24 * 3600)
            guard(); output.mkdir(parents=True)
            original = self.paths
            self.paths = {role: cwd / 'inputs' / self.records[role]['sourcePath'] for role in context['inputRoles']}
            # This synthetic helper's checkpoint role is an additional fixture alias.
            self.paths['checkpointCode'] = cwd / 'inputs' / self.records['checkpointCode']['sourcePath']
            try: result, _, _, _ = self.witness(context, output / 'replay')
            finally: self.paths = original
            raw = json.dumps(result).encode(); receipt = {'outputs': {}}
            for name, value in [('launch.json', {'syntheticOnly': True}), ('started.json', {'pid': 1}), ('stdout.log', raw), ('stderr.log', b'')]:
                pin = production_build.write(output / name, value); kwargs['retain'](output / name, pin)
                if name in ('stdout.log', 'stderr.log'): receipt['outputs'][name] = pin
            pin = production_build.write(output / 'execution.json', receipt); kwargs['retain'](output / 'execution.json', pin)
            return raw, receipt
        actual_validate = repair.validate
        def validate(result, bound, scenario, output, pins):
            actual_validate(result, bound, scenario, output, pins)
            if mutate: mutate(result, bound, scenario, output, pins)
        with patch.object(production_build, 'process', side_effect=process), patch.object(repair, 'validate', side_effect=validate):
            try: repair.run(report, paths, binary, binary_sha, self.root / 'runs', release.local_file, release.digest_file)
            except BaseException:
                self.assertEqual(report['gates'], before); self.assertEqual(len(report['artifacts']), count)
                self.assertFalse(any(role.startswith('forwardRepairOutput-') for role in paths)); raise
        self.assertEqual(report['gates'][repair.GATE]['status'], 'verified')
        for gate in before:
            if gate != repair.GATE: self.assertEqual(report['gates'][gate], before[gate])
        self.assertFalse(report['productionReady'])
        return report

    def test_mock_run_uses_fresh_fixed_coverage_command_and_clears_only_repair(self):
        self.run_mock()

    def test_mock_run_mutated_nested_snapshot_or_process_log_never_registers_outputs(self):
        for target in ('snapshots/000004.json', '../stdout.log'):
            self.setUp()
            def mutate(result, bound, scenario, output, pins):
                with (output / target).open('ab') as stream: stream.write(b' ')
            with self.assertRaises(ValueError): self.run_mock(mutate)

    def test_mock_run_final_registration_mutation_is_atomic(self):
        original = production_build.digest; changed = False
        def digest(path):
            nonlocal changed
            result = original(path)
            if Path(path).name == 'verification.json' and not changed:
                changed = True
                with (Path(path).parent / 'repair/execution.json').open('ab') as stream: stream.write(b' ')
            return result
        with patch.object(production_build, 'digest', side_effect=digest):
            with self.assertRaises(ValueError): self.run_mock()
        self.assertTrue(changed)

    def test_saved_report_without_fresh_identity_pair_cannot_clear_repair(self):
        context = self.prepare()
        report = {'forwardRepairReplay': context, 'gates': {identity.GATE: {'status': 'unverified'}}}
        with patch.object(production_build, 'process') as process:
            with self.assertRaises(ValueError): repair.run(report, self.paths, self.root / 'missing', '00' * 32, self.root, release.local_file, release.digest_file)
            process.assert_not_called()

    def test_native_counter_cannot_be_rebound_with_new_output_hashes(self):
        result, context, output, pins = self.witness()
        derived = result['recoveryReplay']['repairedNativeOperationDerivations'][0]
        def rewrite(path, value):
            path.write_text(json.dumps(value)); pins[str(path)] = release.digest_file(path)
            return pins[str(path)]['sha256']
        report_path = output / derived['reportFile']; native_report = json.loads(report_path.read_text())
        native_report['operationCounter'] = 1
        derived['reportSha256'] = rewrite(report_path, native_report)
        receipt_path = output / derived['processReceiptFile']; receipt = json.loads(receipt_path.read_text())
        receipt['stdoutSha256'] = rewrite(receipt_path.parent / 'stdout.json', native_report)
        derived['processReceiptSha256'] = rewrite(receipt_path, receipt)
        with self.assertRaisesRegex(ValueError, 'native repaired engine/policy/counter/handover'):
            repair.validate(result, context, 'repair', output, pins)

    def test_rehashed_envelope_cannot_substitute_signed_revised_policy_bytes(self):
        result, context, output, pins = self.witness()
        witness = copy.deepcopy(result['recoveryReplay']['suppliedCouncil']['authorizations'][1])
        plan = context['scenarios']['repair']; index = witness['step']
        source = Path(context['inputPaths'][plan['steps'][index]['recoveryRole']])
        raw = source.read_bytes(); altered = raw[:-1] + bytes([raw[-1] ^ 1])
        source.write_bytes(altered); pins[str(source)] = release.digest_file(source)
        witness['envelopeSha256'] = identity.sha(altered)
        envelope_path = output / f'{witness["blockNumber"]}.supplied.escape.scale'
        envelope_path.write_bytes(altered); pins[str(envelope_path)] = release.digest_file(envelope_path)
        witness_path = output / f'{witness["blockNumber"]}.supplied.authorization.json'
        witness_path.write_text(json.dumps(witness)); pins[str(witness_path)] = release.digest_file(witness_path)
        with self.assertRaisesRegex(ValueError, 'signed revised policy bytes differ'):
            repair.authorization(witness, index, result['recoveryReplay']['blocks'], plan, context, identity.OutputEvidence(output, pins))

    def test_audit_policy_must_match_the_runtime_that_actually_audited(self):
        result, context, output, pins = self.witness()
        proof = result['recoveryReplay']['priorPolicyIncomingAudits'][0]
        proof['activePolicy'] = context['policyCommitment']
        proof['finalValidation']['auditedPolicy'] = context['policyCommitment']
        with self.assertRaisesRegex(ValueError, 'historical incoming proof is detached'):
            repair.validate(result, context, 'repair', output, pins)

    def test_direct_137_forward_branch_binds_without_inventing_136_execution(self):
        self.prepare()
        self.put('repairWasm', wasm(137, 132))
        for suffix in ('PolicyScale', 'Binding'):
            self.put('repair137' + suffix, self.paths['repair136' + suffix].read_bytes())
        provenance = json.loads(self.paths['repair136Provenance'].read_text()); provenance['specVersion'] = 137
        provenance['bindingEvidence']['binding'] = {key: self.records['repair137Binding'][key] for key in ('sha256', 'blake2_256')}
        provenance['bindingEvidence']['policy'] = {key: self.records['repair137PolicyScale'][key] for key in ('sha256', 'blake2_256')}
        self.put('repair137Provenance', provenance)
        build = json.loads(self.paths['repair136BuildReceipt'].read_text()); build.update(specVersion=137,
            sourceProvenance=release.digest_file(self.paths['repair137Provenance']), artifacts={'runtime.wasm': release.digest_file(self.paths['repairWasm'])})
        self.put('repair137BuildReceipt', build)
        binding = json.loads(self.paths['native136Binding'].read_text()); binding.update(specVersion=137,
            engineSha256=self.records['repairWasm']['sha256'], engineBlake2=self.records['repairWasm']['blake2_256'])
        self.put('native136Binding', binding)
        self.repair_manifest['nativeOperationRecordersV2'][0]['engineBinding'] = self.ref('native136Binding')
        self.repair_manifest['repairs'] = [self.ref('repairWasm')]
        for step in self.repair_manifest['steps']:
            if step['expectedSpec'] == 136: step['expectedSpec'] = 137
        _, envelope = self.auth_bytes(137, 135, 'Hold', 1, 1, release.policy_identity(self.paths['incomingActivePolicyScale'].read_bytes()))
        self.put('repairEscape', envelope)
        self.stage_repair(137)
        self.repair_manifest['steps'][2]['recovery'] = self.ref('repairStageAuthorization')
        self.repair_manifest['steps'][3]['recovery'] = self.ref('repairStageCode')
        self.repair_manifest['steps'][4]['recovery'] = self.ref('repairEscape')
        self.repair_manifest['suppliedCouncil']['escapes'][1]['destinationSpec'] = 137
        self.repair_manifest['suppliedCouncil']['escapes'][1]['envelopeSha256'] = self.records['repairEscape']['sha256']
        self.save_repair()
        context = self.bind_repair()
        self.assertEqual(context['repairs'], [137])
        self.assertEqual({step['executingSpec'] for step in context['scenarios']['repair']['steps']}, {134, 135, 137, 138})


if __name__ == '__main__':
    unittest.main()
