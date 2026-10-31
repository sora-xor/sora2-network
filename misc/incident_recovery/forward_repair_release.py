"""Fresh forward-repair evidence; no serialized-certificate or readiness promotion."""
import json
from pathlib import Path

import artifact_identity_release as identity
import activation_post_state_release as activation
import hashi_codec as scale
import history_release as history
import incoming_release
import incoming_replay_release as incoming
import repair_staging_release as staging

ROLE = 'forwardRepairReplay'
GATE = 'automaticActivationEscapeAndForwardRepair'
AUTH_FIELDS = ['step', 'blockNumber', 'parentHash', 'envelopeSha256', 'authorizationSha256', 'signingMessageSha256',
    'phase', 'cursor', 'operationCounter', 'storedCodeBlake2', 'executingCodeBlake2', 'destinationCodeBlake2',
    'destinationSpec', 'revisedPolicyCommitment', 'recoveryAnchor', 'signers']
require = identity.require
keys = identity.keys
uint = identity.uint


def bind(root, records, paths, original, identity_context, incoming_context, local_file, digest_file):
    if ROLE not in paths:
        return None
    import release_package as package
    require(identity_context is not None and incoming_context is not None
        and incoming_release.INPUTS <= paths.keys(), 'forward repair requires exact identity pair and complete incoming evidence')
    require(paths[ROLE] == paths['recoveryParentReplay'], 'forward repair must use the same bound incoming replay manifest')
    manifest = history.read(paths[ROLE])
    mandatory = {'schema', 'checkpointHash', 'checkpointCapture', 'policy', 'bootstrap', 'hold', 'finalRuntime',
        'repairs', 'previousGrandpaSet', 'freshGrandpa', 'steps', 'suppliedCouncil'}
    require(mandatory <= set(manifest) <= mandatory | {'nativeOperationRecorder', 'nativeOperationRecordersV2'}, 'unknown or fixture repair fields')
    chain = history.read(paths['chainSpec'])['recovery']
    require(manifest['previousGrandpaSet'] == chain['previousGrandpaSetId'] and type(manifest['previousGrandpaSet']) is int
        and package.grandpa_rows(manifest['freshGrandpa'], hex_only=True) == package.grandpa_rows(chain['grandpaAuthorities']), 'repair consensus differs from release')
    indexed = {path: role for role, path in paths.items()}
    inputs = set(identity_context['inputRoles']) | incoming_release.INPUTS | {ROLE, 'recoveryParentReplay', 'incomingActivePolicyScale'}
    inputs.update(role for role in paths if role.startswith(('incomingWorkShard-', 'incomingReplayInput-')))

    def ref(value, executable=False):
        keys(value, {'file', 'sha256'})
        path = local_file(paths[ROLE].parent, value['file'])
        require(path in indexed, 'repair reference absent from bound incoming graph')
        role = indexed[path]
        pin = digest_file(path)
        require(pin['sha256'] == value['sha256'] and all(records[role][key] == pin[key] for key in pin), 'repair input changed')
        if executable:
            require(records[role]['executable'] and path.stat().st_mode & 0o777 == 0o555 and path.stat().st_nlink == 1, 'repair native executable requires immutable independent copy')
        inputs.add(role)
        return role

    wasm = dict(identity_context['wasm'])
    policies = {'134': {'role': 'policyScale', **original}, '135': {'role': 'policyScale', **original}}
    repairs = []
    for value in manifest['repairs']:
        role = ref(value)
        version = package.wasm_identity(paths[role].read_bytes())['specVersion']
        require(version in (136, 137) and version not in repairs, 'repair version duplicate or unsupported')
        repairs.append(version)
        prefix = 'repair' + str(version)
        inputs.update(prefix + suffix for suffix in ('PolicyScale', 'Binding', 'Provenance', 'BuildReceipt'))
        policy_role = prefix + 'PolicyScale'
        policies[str(version)] = {'role': policy_role, **package.policy_identity(paths[policy_role].read_bytes())}
        wasm[str(version)] = {'role': role, 'sha256': records[role]['sha256'], 'blake2': records[role]['blake2_256']}
    require(repairs in ([136], [137], [136, 137]), 'repair branch must be forward and complete')
    require(policies[str(repairs[-1])]['commitment'] == incoming_context['activePolicy']['commitment'], 'last repair does not carry final active policy')
    policies['138'] = policies[str(repairs[-1])]
    configurations = ([manifest['nativeOperationRecorder']] if 'nativeOperationRecorder' in manifest else []) + manifest.get('nativeOperationRecordersV2', [])
    native = {}
    for config in configurations:
        keys(config, {'executable', 'engineBinding'})
        executable, binding_role = ref(config['executable'], True), ref(config['engineBinding'])
        binding = history.read(paths[binding_role])
        version = binding.get('specVersion')
        require(type(version) is int and version in [134, *repairs] and str(version) not in native, 'unexpected or duplicate native repair engine')
        policy = policies[str(version)]
        expected = {'schema': 'sora-nexus-native-operation-engine-binding-v' + ('1' if version == 134 else '2'),
            'engineSha256': wasm[str(version)]['sha256'], 'engineBlake2': wasm[str(version)]['blake2'],
            'policySha256': records[policy['role']]['sha256'], 'policyCommitment': policy['commitment'],
            'sdk': 'polkadot-stable2603-3', 'specVersion': version, 'transactionVersion': 131 if version == 134 else 132,
            'fixtureOnly': False, 'productionReady': False}
        if version != 134:
            expected.update(originalPolicySha256=records['policyScale']['sha256'], originalPolicyCommitment=original['commitment'], templateCarriesReviewedSdkAndTestPathRepairs=True)
        require(all(binding.get(key) == value and type(binding.get(key)) is type(value) for key, value in expected.items())
            and all(key in binding for key in ('runtimeFeatureProfile', 'hostLockAmendment')), 'native engine code/active policy/original policy binding differs')
        native[str(version)] = {'executableRole': executable, 'bindingRole': binding_role,
            'executableSha256': records[executable]['sha256'], 'bindingSha256': records[binding_role]['sha256']}
    steps = manifest['steps']
    require(4 <= len(steps) <= identity.MAX_STEPS, 'repair execution step bound')
    details, dynamic, escapes = [], [], []
    version, counter, prior_slot = 134, 0, 0
    for index, step in enumerate(steps):
        required = {'slot', 'timestamp', 'authorityIndex', 'expectedSpec', 'expectedPhase'}
        require(required <= set(step) <= required | {'recovery', 'rejectedRecovery', 'rejectPreviousEscapes'}, 'unknown repair step fields')
        require(uint(step['slot']) > prior_slot and uint(step['timestamp']) > 0 and uint(step['authorityIndex'], 2**32 - 1) < len(manifest['freshGrandpa']), 'repair consensus/time bounds')
        require(index != 0 or step['slot'] == chain['firstRecoverySlot'], 'repair first slot differs from release')
        prior_slot = step['slot']
        negatives = step.get('rejectedRecovery', [])
        require(isinstance(negatives, list) and len(negatives) <= 128 and type(step.get('rejectPreviousEscapes', False)) is bool, 'negative repair probes malformed')
        negative_roles = [ref(value) for value in negatives]
        recovery = ref(step['recovery']) if step.get('recovery') is not None else None
        tag, operation_index = identity.action(paths[recovery].read_bytes()) if recovery else (None, None)
        require((recovery is not None) == (version != 138), 'repair envelope presence differs from executing runtime')
        next_version = step['expectedSpec']
        require(type(next_version) is int and str(next_version) in wasm and isinstance(step['expectedPhase'], str), 'unknown repair resulting version')
        before_counter = counter
        if tag == 4:
            destination = [135, *repairs][counter] if counter < 1 + len(repairs) else None
            require(next_version == destination and version == (134 if counter == 0 else [135, *repairs][counter - 1]), 'escape must install the next approved forward runtime')
            counter += 1
            require(step['expectedPhase'] == ('Hold' if next_version == 135 else 'Applying'), 'escape phase differs')
            escapes.append({'step': index, 'envelopeSha256': records[recovery]['sha256'], 'destinationSpec': next_version})
        elif next_version == 138 and version != 138:
            require(version == repairs[-1] and tag in (0, 3, 5) and step['expectedPhase'] == 'Complete' and counter == 1 + len(repairs), 'final activation must follow the complete repair branch')
        else:
            require(next_version == version and (step['expectedPhase'] == 'Hold' and tag in (0, 3, 12) if version == 135 else step['expectedPhase'] in ('Applying', 'Auditing', 'Ready', 'Complete', 'Failed')), 'unapproved runtime/phase transition')
            require(version != 138 or step['expectedPhase'] == 'Complete', 'ordinary final runtime phase')
        require(not step.get('rejectPreviousEscapes', False) or before_counter > 0, 'escape replay probe before an escape')
        if tag in (6, 7):
            require(str(version) in native, 'dynamic work lacks exact native engine')
            dynamic.append(index)
        details.append({'step': step, 'recoveryRole': recovery, 'negativeRoles': negative_roles, 'action': tag,
            'operationIndex': operation_index, 'executingSpec': version, 'counterBefore': before_counter, 'counterAfter': counter,
            'policy': policies[str(version)], 'resultPolicy': policies[str(next_version)]})
        version = next_version
    require(counter == 1 + len(repairs) and version == 138 and {detail['executingSpec'] for detail in details} == {134, 135, 138, *repairs}, 'every approved repair runtime must execute, not merely install')
    repair_staging = staging.bind(details, wasm, policies, original, identity_context['council'], paths)
    keys(manifest['suppliedCouncil'], {'escapes', 'expectedTerminal'})
    require(manifest['suppliedCouncil']['escapes'] == escapes, 'escape list/order/raw envelope differs')
    terminal = manifest['suppliedCouncil']['expectedTerminal']
    final_policy = policies['138']
    keys(terminal, {'specVersion', 'phase', 'cursor', 'auditCursor', 'operationCounter', 'activePolicyCommitment', 'stateRoot', 'terminalAuditBlake2', 'ordinaryBlocks'})
    for key in ('specVersion', 'cursor', 'auditCursor', 'operationCounter', 'ordinaryBlocks'):
        uint(terminal[key])
    require(terminal['specVersion'] == 138 and terminal['phase'] == 'Complete' and terminal['cursor'] == final_policy['workChunks']
        and terminal['auditCursor'] == final_policy['auditChunks'] and terminal['operationCounter'] == counter
        and terminal['activePolicyCommitment'] == '0x' + final_policy['commitment'] and terminal['ordinaryBlocks'] == sum(detail['executingSpec'] == 138 for detail in details), 'final repair terminal differs from complete active policy')
    for key in ('stateRoot', 'terminalAuditBlake2'):
        identity.digest_hex(terminal[key], True)
    final_raw = paths[final_policy['role']].read_bytes()
    require(terminal['terminalAuditBlake2'] == '0x' + identity.blake(bytes.fromhex(final_policy['commitment']) + final_raw[-32:] + scale.u32(final_policy['workChunks'])), 'terminal audit is not the exact final policy/work tuple')
    require(dynamic and any(details[index]['executingSpec'] in repairs for index in dynamic), 'this hook requires actual repaired native operation comparison')
    require(set(native) == {str(details[index]['executingSpec']) for index in dynamic}, 'unused or missing native recorder')
    for role in inputs:
        require(role in records, 'missing repair input graph role')
        pin = digest_file(paths[role])
        require(all(records[role][key] == pin[key] for key in pin), 'complete repair input graph changed')
    context = {**identity_context, 'wasm': wasm, 'policies': policies, 'repairs': repairs,
        'inputRoles': sorted(inputs), 'incomingContext': incoming_context, 'originalPolicy': original,
        'artifactBindings': {role: records[role]['sha256'] for role in records},
        'artifactBlakes': [record['blake2_256'] for record in records.values()],
        'scenarios': {'repair': {'manifestRole': ROLE, 'manifestSha256': records[ROLE]['sha256'], 'manifest': manifest,
            'steps': details, 'native': native, 'dynamicSteps': dynamic, 'escapes': escapes, 'repairStaging': repair_staging,
            'expectedArtifacts': incoming_context['expectedArtifacts']}}}
    return context


def authorization(witness, index, rows, plan, context, evidence):
    import release_package as package
    keys(witness, AUTH_FIELDS)
    detail = plan['steps'][index]
    require(index > 0, 'first-child escape has no established recovery anchor')
    before = rows[index - 1] if index else {'hash': scale.CHECKPOINT_HASH, 'phase': 'Applying', 'cursor': 0}
    version, destination = detail['executingSpec'], detail['step']['expectedSpec']
    expected = {'recoveryAnchor': rows[0]['hash'], 'step': index, 'blockNumber': scale.CHECKPOINT + index + 1, 'parentHash': before['hash'],
        'phase': before['phase'], 'cursor': before['cursor'], 'operationCounter': detail['counterBefore'],
        'storedCodeBlake2': '0x' + (context['checkpointCodeBlake2'] if version == 134 else context['wasm'][str(version)]['blake2']),
        'executingCodeBlake2': '0x' + context['wasm'][str(version)]['blake2'],
        'destinationCodeBlake2': '0x' + context['wasm'][str(destination)]['blake2'], 'destinationSpec': destination,
        'revisedPolicyCommitment': '0x' + ('00' * 32 if destination == 135 else context['policies'][str(destination)]['commitment'])}
    require(all(witness[key] == value and type(witness[key]) is type(value) for key, value in expected.items()), 'escape authorization observed context differs')
    signers = witness['signers']
    require(isinstance(signers, list) and len(signers) == len(set(signers)) and len(signers) > len(context['council']) // 2
        and all(signer in ['0x' + account for account in context['council']] for signer in signers), 'repair quorum membership/duplicates')
    phase = ['Applying', 'Auditing', 'Ready', 'Hold', 'Complete', 'Failed'].index(witness['phase'])
    raw = (bytes.fromhex(context['policyCommitment']) + bytes([phase]) + scale.u32(witness['cursor'])
        + witness['operationCounter'].to_bytes(8, 'little')
        + b''.join(bytes.fromhex(witness[key][2:]) for key in ('storedCodeBlake2', 'executingCodeBlake2', 'destinationCodeBlake2'))
        + scale.u32(destination) + bytes.fromhex(witness['revisedPolicyCommitment'][2:])
        + package.authorization_domain_scale(context['originalPolicy']['authorizationDomain'])
        + bytes.fromhex(witness['recoveryAnchor'][2:]))
    message = scale.vec(b'sora-nexus/recovery-forward-upgrade/v2') + bytes.fromhex(scale.GENESIS_HASH[2:]) + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + raw
    prefix = f'{witness["blockNumber"]}.supplied.'
    require(evidence.document(prefix + 'authorization.json') == witness
        and evidence.pinned(evidence.local(prefix + 'authorization.scale'), witness['authorizationSha256']) == raw
        and evidence.pinned(evidence.local(prefix + 'signing-message.scale'), witness['signingMessageSha256']) == message, 'raw signed repair authorization differs')
    envelope = evidence.pinned(Path(context['inputPaths'][detail['recoveryRole']]), witness['envelopeSha256'], identity.MAX_ENVELOPE)
    require(evidence.pinned(evidence.local(prefix + 'escape.scale'), witness['envelopeSha256'], identity.MAX_ENVELOPE) == envelope
        and envelope[:10] == b'snxrec01\x01\x04' and envelope[10:10 + len(raw)] == raw, 'escape envelope authorization differs')
    reader = scale.Reader(envelope[10 + len(raw):])
    count = reader.compact()
    require(count == len(signers), 'raw signer count differs')
    actual = []
    for _ in range(count):
        actual.append('0x' + reader.take(32).hex())
        kind = reader.take(1)[0]
        require(kind in (0, 1, 2), 'unknown council signature kind')
        reader.take(65 if kind == 2 else 64)
    require(actual == signers, 'raw council signer order differs')
    require(reader.take(1) == (b'\x00' if destination == 135 else b'\x01'), 'signed work policy option differs')
    if destination != 135:
        policy = evidence.pinned(Path(context['inputPaths'][context['policies'][str(destination)]['role']]), maximum=2 * 1024**2)
        require(reader.take(len(policy)) == policy, 'signed revised policy bytes differ')
    reader.end()


def validate(result, context, scenario, output, pins):
    require(scenario == 'repair', 'unknown repair scenario')
    report_context = {'policy': context['originalPolicy'], 'incomingRecoveryReplay': context['incomingContext'],
        'artifacts': [{'blake2_256': value} for value in context['artifactBlakes']]}
    require(incoming_release.valid_receipt(result, report_context, context['artifactBindings'], history.ROOT), 'fresh opaque incoming certificate/coverage/retained audits failed')
    validate_baseline(result['recoveryReplay'], context, scenario, output, pins)
    activation.block_weight.join_metadata(result['recoveryReplay'].get('activationValidation'), identity.OutputEvidence(output, pins))
    observed = activation.validate(result['recoveryReplay'], context, scenario)
    require(observed['status'] == 'verified',
        'repaired final runtime activation gate failed: ' + observed['reason'])
    return observed


def validate_baseline(result, context, scenario, output, pins):
    """Validate raw baseline witnesses emitted by a fresh pinned native process.

    This does not deserialize or establish an incoming certificate. The caller
    must separately require its fresh native opaque-backed binding.
    """
    require(scenario == 'repair', 'unknown repair scenario')
    plan = context['scenarios'][scenario]
    expected_fields = {'schema', 'manifestSha256', 'captureManifestSha256', 'policyCommitment', 'artifactSha256', 'blocks',
        'automaticActivationObserved', 'councilHoldObserved', 'forwardRepairObserved', 'ordinaryRuntimeBlocks',
        'livenessGraceObserved', 'feeClockShiftObserved', 'bridgeHaltRetained', 'nativeExecution',
        'consensusSealsVerified', 'operatorApprovalVerified', 'productionReady', 'publicTestCouncilFixture',
        'publicTestHashiFixture', 'fixtureGovernanceStateUnchanged', 'fixtureStateExpectationChecks',
        'nativeOperationDerivations', 'repairedNativeOperationDerivations', 'replayTranscriptSha256',
        'suppliedCouncil', 'priorPolicyIncomingAudits', 'incomingReplayBinding', 'repairStagingAuthorizations'}
    if 'activationValidation' in result:
        expected_fields.add('activationValidation')
    keys(result, expected_fields)
    require(result['schema'] == 'sora-nexus-supplied-council-maintenance-replay-witness-v1'
        and result['manifestSha256'] == plan['manifestSha256'] and result['captureManifestSha256'] == context['captureManifestSha256']
        and result['policyCommitment'] == context['policyCommitment'] and result['artifactSha256'] == plan['expectedArtifacts'], 'repair replay identity differs')
    require(all(result[key] is True for key in ('automaticActivationObserved', 'councilHoldObserved', 'forwardRepairObserved', 'feeClockShiftObserved', 'bridgeHaltRetained', 'nativeExecution'))
        and all(result[key] is False for key in ('consensusSealsVerified', 'operatorApprovalVerified', 'productionReady', 'publicTestCouncilFixture', 'publicTestHashiFixture'))
        and result['fixtureGovernanceStateUnchanged'] is None and type(result['fixtureStateExpectationChecks']) is int and result['fixtureStateExpectationChecks'] == 0
        and type(result['livenessGraceObserved']) is bool, 'repair report scope/transition evidence differs')
    rows = result['blocks']
    require(isinstance(rows, list) and len(rows) == len(plan['steps']), 'repair block count differs')
    evidence = identity.OutputEvidence(output, pins)
    evidence.journal(rows)
    transcript = identity.sha(b'sora-nexus-maintenance-transcript-v1' + plan['manifestSha256'].encode() + context['captureManifestSha256'].encode())
    lineage = scale.MAGIC + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + bytes.fromhex(context['policyCommitment'])
    parent, prior_transcripts, authored_blocks, seen = scale.CHECKPOINT_HASH, {}, {}, set()
    for index, (row, detail) in enumerate(zip(rows, plan['steps'])):
        keys(row, identity.BLOCK_FIELDS)
        step, version = detail['step'], detail['executingSpec']
        number = scale.CHECKPOINT + index + 1
        require(type(row['number']) is int and row['number'] == number and row['executingCodeBlake2'] == context['wasm'][str(version)]['blake2']
            and type(row['resultingSpec']) is int and row['resultingSpec'] == step['expectedSpec'] and row['phase'] == step['expectedPhase'], 'actual repair executing code/version/phase differs')
        require(row['independentImportRootMatches'] is True and row['diskRestartRootMatches'] is True
            and row['maintenanceSessionFrozen'] is (version != 138) and row['generatedAuthorizationSha256'] is None, 'repair author/import/restore scope differs')
        require(uint(row['operationCounter']) == detail['counterAfter'] and uint(row['cursor'], detail['resultPolicy']['workChunks']) >= 0, 'repair cursor/counter differs')
        if detail['action'] == 4:
            require(row['cursor'] == ((rows[index - 1]['cursor'] if index else 0) if step['expectedSpec'] == 135 else 0), 'escape did not preserve/reset its cursor')
        else:
            require(row['cursor'] >= (rows[index - 1]['cursor'] if index else 0), 'cursor regressed without repair')
        rejected = 2 + len(detail['negativeRoles']) + detail['counterBefore'] if version != 138 else 0
        require(type(row['forbiddenExtrinsicsRejected']) is int and row['forbiddenExtrinsicsRejected'] == rejected, 'escape replay/ordinary transaction rejection count differs')
        raw = evidence.pinned(evidence.local(f'{number}.block.scale'), row['blockSha256'], 2 * 1024**2)
        block = identity.block_header(raw)
        if index == 0:
            require(not any(tag == 5 for tag, _, _ in block['logs']), 'rehearsal branch anchor must be unsealed and distinct from valid production BABE blocks')
        require(block['number'] == number and block['parentHash'] == parent and block['hash'] == row['hash'] and block['stateRoot'] == row['stateRoot'], 'repair block bytes/ancestry/state differ')
        require([data for tag, engine, data in block['logs'] if tag == 6 and engine == b'BABE'] == [b'\x02' + scale.u32(step['authorityIndex']) + step['slot'].to_bytes(8, 'little')]
            and [data for tag, _, data in block['logs'] if tag == 0 and data.startswith(scale.MAGIC)] == [lineage], 'repair BABE or immutable lineage differs')
        extrinsics = [b'\x04\x01\x00' + scale.compact(step['timestamp'])]
        if detail['recoveryRole'] is not None:
            extrinsics.append(b'\x04\x00\x00' + scale.vec(evidence.pinned(Path(context['inputPaths'][detail['recoveryRole']]), maximum=identity.MAX_ENVELOPE)))
        require(block['extrinsics'] == extrinsics, 'repair authored raw envelope differs')
        prior_transcripts[number], authored_blocks[number] = transcript, raw
        transcript = identity.sha(transcript.encode() + json.dumps({key: row[key] for key in identity.BLOCK_FIELDS}, separators=(',', ':'), ensure_ascii=False).encode())
        parent = row['hash']; seen.add(version)
    require(transcript == result['replayTranscriptSha256'] and seen == {134, 135, 138, *context['repairs']}, 'repair transcript or actual execution coverage differs')
    for proof in result['priorPolicyIncomingAudits']:
        consumed = proof['consumingBlock']['block']['number'] - scale.CHECKPOINT - 1
        audited = proof['block']['number'] - scale.CHECKPOINT - 1
        require(0 <= consumed < audited < len(rows), 'historical incoming proof block bounds')
        consume_step, audit_step = plan['steps'][consumed], plan['steps'][audited]
        require(consume_step['action'] == 6 and audit_step['action'] == 9
            and proof['consumedPolicy'] == consume_step['policy']['commitment']
            and proof['activePolicy'] == audit_step['policy']['commitment']
            and proof['consumingBlock']['activePolicy'] == consume_step['policy']['commitment'], 'historical incoming proof is detached from executed policy/actions')
    terminal = plan['manifest']['suppliedCouncil']['expectedTerminal']
    require(result['ordinaryRuntimeBlocks'] == terminal['ordinaryBlocks'] and type(result['ordinaryRuntimeBlocks']) is int
        and rows[-1]['stateRoot'] == terminal['stateRoot'] and rows[-1]['cursor'] == terminal['cursor'] and rows[-1]['phase'] == 'Complete', 'repair final state differs')
    council = result['suppliedCouncil']
    keys(council, {'schema', 'authorizations', 'terminal', 'signaturesVerifiedAgainstPinnedPolicy', 'automaticActivationObserved',
        'publicTestCouncilGenerated', 'councilInventoryApprovalVerified', 'sourceProvenanceVerified', 'productionReady', 'publicDynamicFixture'})
    require(council['terminal'] == terminal and council['signaturesVerifiedAgainstPinnedPolicy'] is True
        and council['automaticActivationObserved'] is True and council['schema'] == 'sora-nexus-supplied-council-replay-witness-v1'
        and all(council[key] is False for key in ('publicTestCouncilGenerated', 'councilInventoryApprovalVerified', 'sourceProvenanceVerified', 'productionReady', 'publicDynamicFixture')), 'forward repair quorum/scope evidence differs')
    require(isinstance(council['authorizations'], list) and len(council['authorizations']) == len(plan['escapes']), 'repair authorization count differs')
    for witness, escape in zip(council['authorizations'], plan['escapes']):
        require(witness['envelopeSha256'] == escape['envelopeSha256'], 'committed escape envelope differs')
        authorization(witness, escape['step'], rows, plan, context, evidence)
    staging.validate(result['repairStagingAuthorizations'], rows, plan, context, evidence, council['authorizations'])
    validate_native(result, context, plan, evidence, prior_transcripts, authored_blocks, council['authorizations'])


def validate_native(result, context, plan, evidence, transcripts, blocks, authorizations):
    for field, repaired in [('nativeOperationDerivations', False), ('repairedNativeOperationDerivations', True)]:
        actual = result[field]
        expected = [index for index in plan['dynamicSteps'] if (plan['steps'][index]['executingSpec'] != 134) == repaired]
        require(isinstance(actual, list) and len(actual) == len(expected), 'native derivation count differs')
        for derived, index in zip(actual, expected):
            detail = plan['steps'][index]; number = scale.CHECKPOINT + index + 1
            version = str(detail['executingSpec']); native = plan['native'][version]; policy = detail['policy']
            require(derived.get('block') == number and derived.get('operationIndex') == detail['operationIndex']
                and derived.get('executableSha256') == native['executableSha256'] and derived.get('engineBindingSha256') == native['bindingSha256']
                and derived.get('parentReplayTranscriptSha256') == transcripts[number], 'native repaired work detached from actual replay')
            require(all(derived.get(key) is True for key in ('joinedToVerifiedReplayPrefix', 'exactAuthoredBlockBytesMatch', 'completeOperationDeltaCompared', 'nativeTerminalSnapshotIndependentlyRestored'))
                and all(derived.get(key) is False for key in ('nativeSourceBuildAuthenticityVerified', 'sourceFinalityVerified', 'economicPolicyIndependentlyVerified', 'fixtureOnly', 'productionReady')), 'native derivation scope changed')
            prefix = f'native-operation-{number}/'
            require(derived.get('reportFile') == prefix + 'recorded/report.json' and derived.get('processReceiptFile') == prefix + 'process.json', 'native output path differs')
            report = evidence.document(derived['reportFile'], derived['reportSha256'])
            receipt = evidence.document(derived['processReceiptFile'], derived['processReceiptSha256'])
            expected_report = {'schema': 'sora-nexus-native-operation-expectations-v' + ('2' if repaired else '1'),
                'engineSha256': context['wasm'][version]['sha256'], 'policyCommitment': '0x' + policy['commitment'],
                'blockSha256': result['blocks'][index]['blockSha256'], 'stateRoot': result['blocks'][index]['stateRoot'],
                'fixtureOnly': False, 'productionReady': False, 'nativeWasmOperationParityVerified': True,
                'independentImportVerified': True, 'freshDiskRestoreVerified': True, 'completeTopWriteSetCompared': True,
                'consensusPrefixWaiver': False, 'compiledPolicyOverrideUsed': False, 'configurationInjected': False}
            if repaired:
                handover = [value for value in authorizations if value['destinationSpec'] == int(version)]
                require(len(handover) == 1, 'native repair lacks its actual accepted handover')
                pending_handover = index == handover[0]['step'] + 1
                expected_report.update(originalPolicyCommitment='0x' + context['policyCommitment'], originalPolicySha256=context['artifactBindings']['policyScale'],
                    activePolicySha256=context['artifactBindings'][policy['role']], specVersion=int(version), transactionVersion=132,
                    operationCounter=detail['counterBefore'], parentRepairAuthorizationSha256=handover[0]['authorizationSha256'] if pending_handover else None, repairHandoverLifecycleChecked=True)
                work_context = evidence.document(prefix + 'recovery-context.json', report['recoveryContextSha256'])
                accepted = [{key: value[key] for key in AUTH_FIELDS} for value in authorizations if value['step'] < index]
                parent = result['blocks'][index - 1]
                raw_handover = evidence.pinned(evidence.local(f'{handover[0]["blockNumber"]}.supplied.authorization.scale'), handover[0]['authorizationSha256'])
                require(work_context == {'schema': 'sora-nexus-native-verified-work-context-v2', 'parentHash': parent['hash'],
                    'parentStateRoot': parent['stateRoot'], 'parentSnapshotManifestSha256': parent['diskSnapshotManifestSha256'],
                    'priorTranscriptSha256': transcripts[number], 'operationCounter': detail['counterBefore'],
                    'acceptedEscapesSha256': identity.sha(json.dumps(accepted, separators=(',', ':')).encode()),
                    'parentRepairAuthorization': '0x' + raw_handover.hex() if pending_handover else None}, 'native repaired work context/accepted escape prefix differs')
            require(all(report.get(key) == value and type(report.get(key)) is type(value) for key, value in expected_report.items()), 'native repaired engine/policy/counter/handover differs')
            binding = history.parse(evidence.pinned(Path(context['inputPaths'][native['bindingRole']])))
            require(history.parse(report['engineBinding']) == binding, 'native embedded engine binding differs')
            evidence.native_snapshot(prefix + 'recorded/', report, blocks[number], result['blocks'][index]['stateRoot'])
            require(receipt.get('schema') == 'sora-nexus-native-operation-process-v1' and type(receipt.get('pid')) is int and receipt['pid'] > 0
                and type(receipt.get('exitCode')) is int and receipt['exitCode'] == 0 and receipt.get('success') is True and receipt.get('stopReason') is None
                and receipt.get('executableSha256') == native['executableSha256'] and receipt.get('executable') == context['inputPaths'][native['executableRole']]
                and receipt.get('sourceFinalityVerified') is False and receipt.get('productionReady') is False, 'native repair process receipt differs')
            for filename, field in [('stdout.json', 'stdoutSha256'), ('stderr.log', 'stderrSha256'), ('manifest.json', 'manifestSha256')]:
                evidence.pinned(evidence.local(prefix + filename), receipt[field])
            require(evidence.document(prefix + 'stdout.json') == report, 'native repair stdout differs')


def run(report, paths, binary, expected_sha256, parent, local_file, digest_file):
    if 'forwardRepairReplay' not in report:
        return
    require(report['gates'][identity.GATE]['status'] == 'verified'
        and report['gates'][identity.GATE]['verifierSha256'] == expected_sha256, 'fresh exact identity pair must pass first')
    def command(verifier, owned, plan, output):
        arguments = incoming_release.arguments({role: Path(path) for role, path in owned.items()})
        require(arguments is not None and arguments[0] == 'verify-incoming-coverage'
            and arguments[arguments.index('--recovery-replay') + 1] == owned[ROLE], 'fixed repair coverage command differs')
        return [str(verifier), *arguments, '--recovery-output', str(output / 'replay')]
    profile = {'context': 'forwardRepairReplay', 'gate': GATE, 'label': 'forward-repair',
        'schema': 'sora-nexus-forward-repair-release-v1', 'failureSchema': 'sora-nexus-forward-repair-release-failure-v1',
        'outputRole': 'forwardRepairOutput', 'directoryField': 'forwardRepairOutputDirectory', 'scenarios': ('repair',),
        'sources': [Path(__file__).resolve(), Path(incoming_release.__file__).resolve(), Path(activation.__file__).resolve(), Path(activation.block_weight.__file__).resolve(), Path(staging.__file__).resolve()],
        'command': command, 'validate': validate, 'select': lambda result: result['recoveryReplay'],
        'versions': lambda context: [134, 135, *context['repairs'], 138],
        'reason': 'Fresh exact-Wasm hold, supplied-quorum forward repair and automatic final activation, with raw authorization/counter/policy joins, native comparisons and opaque historical incoming audit preservation; consensus/build/operator and other gates remain separate'}
    return identity.run_owned(report, paths, binary, expected_sha256, parent, local_file, digest_file, profile)
