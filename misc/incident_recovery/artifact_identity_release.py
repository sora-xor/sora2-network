"""Fresh exact-Wasm identity evidence; no consensus or release-readiness promotion.

The trusted, explicitly pinned native maintenance CLI owns Wasm Core_version,
author/import execution, authenticated escape and disk-state reconstruction.
This module binds its fresh inputs and outputs to this package, never admits an
archived report or deserializes an execution certificate into native state.
"""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile

import hashi_codec as scale
import history_release as history
import execution_release as encoding

ROLES = {'artifactActivationReplay': 'activation', 'artifactHoldReplay': 'hold'}
GATE = 'executingArtifactIdentity'
MAX_STEPS = 4096
MAX_REPORT = 32 * 1024**2
MAX_ENVELOPE = 576 * 1024
MAX_GRAPH = 128 * 1024**3
MAX_FILES = 65536
REPLAY_TIMEOUT = 24 * 3600
BLOCK_FIELDS = ['number', 'hash', 'stateRoot', 'executingCodeBlake2', 'resultingSpec',
    'phase', 'cursor', 'operationCounter', 'blockSha256', 'diskSnapshotManifestSha256',
    'forbiddenExtrinsicsRejected', 'maintenanceSessionFrozen', 'independentImportRootMatches',
    'diskRestartRootMatches', 'generatedAuthorizationSha256']


def require(ok, message):
    scale.require(ok, 'Executing artifact identity: ' + message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def blake(raw):
    return hashlib.blake2b(raw, digest_size=32).hexdigest()


def keys(value, expected):
    require(isinstance(value, dict) and set(value) == set(expected), 'unexpected fields')


def uint(value, maximum=2**64 - 1):
    require(type(value) is int and 0 <= value <= maximum, 'unsigned integer required')
    return value


def digest_hex(value, prefixed=False):
    return history.digest_hex(value, prefixed)


def policy_controls(raw):
    reader = scale.Reader(raw)
    reader.take(68)
    mapping = [(reader.take(32).hex(), reader.take(32).hex()) for _ in range(reader.compact())]
    council = [reader.take(32).hex() for _ in range(reader.compact())]
    return mapping, council


def action(raw):
    require(10 <= len(raw) <= MAX_ENVELOPE and raw[:9] == b'snxrec01\x01', 'recovery envelope identity/size')
    tag = raw[9]
    require(tag <= 11, 'unknown recovery action')
    index = None
    if tag in (6, 7):
        reader = scale.Reader(raw[10:])
        payload = reader.vector(512 * 1024)
        reader.end()
        require(len(payload) >= 4, 'dynamic operation index absent')
        index = int.from_bytes(payload[:4], 'little')
    return tag, index


def bind(root, records, paths, local_file, digest_file):
    if not (ROLES.keys() & paths.keys()):
        return None
    from production_build import PUBLIC_SIGNING_KEYS
    from release_package import grandpa_key, grandpa_rows
    required = set(ROLES) | {'checkpointCaptureManifest', 'policyScale', 'operatorsScale',
        'bootstrapWasm', 'holdWasm', 'finalWasm', 'bootstrapBinding', 'holdBinding',
        'bootstrapProvenance', 'holdProvenance', 'finalProvenance', 'chainSpec'}
    require(required <= paths.keys(), 'both replay scenarios and complete release bindings required')
    for role in ['bootstrapProvenance', 'holdProvenance', 'finalProvenance']:
        provenance = history.read(paths[role])
        require(provenance.get('fixtureOnly') is not True and 'fixture' not in str(provenance.get('schema', '')).lower(), 'fixture source cannot establish release identity')
    mapping, council = policy_controls(paths['policyScale'].read_bytes())
    chain = history.read(paths['chainSpec'])['recovery']
    require(not PUBLIC_SIGNING_KEYS.intersection([new for _, new in mapping] + council + [grandpa_key(key).hex() for key in chain['approvedGrandpaKeys']]), 'known public signing identity rejected across roles')
    policy = blake(paths['policyScale'].read_bytes())
    indexed = {path: role for role, path in paths.items()}
    input_roles = set(required)
    counted = set()
    aggregate = 0

    def add(path, expected=None, executable=False):
        nonlocal aggregate
        require(path.resolve(strict=True) == path and not path.is_symlink(), 'input alias')
        actual = digest_file(path)
        if expected is not None:
            digest_hex(expected)
            require(actual['sha256'] == expected, 'nested artifact digest differs')
        if path not in counted:
            aggregate += actual['bytes']
            counted.add(path)
        require(aggregate <= MAX_GRAPH and len(records) < 32768, 'input graph bound')
        if executable:
            require(path.stat().st_nlink == 1 and path.stat().st_mode & 0o777 == 0o555, 'native recorder needs an independent mode555 pin')
        if path not in indexed:
            role = f'artifactIdentityInput-{len(indexed):05d}'
            require(role not in records, 'reserved input role collision')
            records[role] = {'role': role, **actual, 'object': 'objects/' + actual['sha256'], 'sourcePath': path.relative_to(root).as_posix(), 'executable': executable}
            paths[role] = path
            indexed[path] = role
        role = indexed[path]
        require(all(records[role][key] == actual[key] for key in actual), 'existing artifact changed')
        if executable:
            require(records[role]['executable'], 'native recorder executable role differs')
        input_roles.add(role)
        return role

    capture_path = paths['checkpointCaptureManifest']
    capture = history.read(capture_path)
    require(capture.get('schema') == 'sora-nexus-checkpoint-capture-v1' and capture.get('complete') is True
        and capture.get('checkpoint', {}).get('hash') == scale.CHECKPOINT_HASH
        and capture['checkpoint'].get('stateRoot') == history.ROOT, 'complete exact checkpoint capture required')
    references = dict(capture.get('artifacts', {}))
    for trie in [capture.get('top', {}), *capture.get('children', [])]:
        require(isinstance(trie.get('pages'), list), 'capture page graph missing')
        for page in trie['pages']:
            require(page['file'] not in references, 'duplicate capture graph reference')
            references[page['file']] = {'sha256': page['sha256']}
    require(references, 'empty checkpoint graph')
    for name, pin in references.items():
        role = add(local_file(capture_path.parent, name), pin['sha256'])
        require('bytes' not in pin or records[role]['bytes'] == pin['bytes'], 'capture length mismatch')
    deployed = local_file(capture_path.parent, 'runtime-code.wasm')
    require(deployed in indexed, 'checkpoint runtime is not captured')
    wasm = {str(version): {'role': role, 'sha256': records[role]['sha256'], 'blake2': records[role]['blake2_256']}
        for version, role in [(134, 'bootstrapWasm'), (135, 'holdWasm'), (138, 'finalWasm')]}
    from release_package import policy_identity
    policy_counts = policy_identity(paths['policyScale'].read_bytes())
    context = {'policyCommitment': policy, 'council': council, 'workChunks': policy_counts['workChunks'], 'auditChunks': policy_counts['auditChunks'], 'captureManifestSha256': records['checkpointCaptureManifest']['sha256'],
        'checkpointCodeBlake2': records[indexed[deployed]]['blake2_256'], 'wasm': wasm, 'scenarios': {}, 'productionReady': False}
    for manifest_role, scenario in ROLES.items():
        source = paths[manifest_role]
        manifest = history.read(source)
        mandatory = {'schema', 'checkpointHash', 'checkpointCapture', 'policy', 'bootstrap', 'hold', 'finalRuntime', 'previousGrandpaSet', 'freshGrandpa', 'steps'}
        allowed = mandatory | ({'nativeOperationRecorder'} if scenario == 'activation' else {'suppliedCouncil'})
        require(isinstance(manifest, dict) and mandatory <= set(manifest) <= allowed, 'fixture/repair/unknown manifest fields rejected')
        require(manifest['schema'] == ('sora-nexus-maintenance-replay-v1' if scenario == 'activation' else 'sora-nexus-supplied-council-maintenance-replay-v1') and manifest['checkpointHash'] == scale.CHECKPOINT_HASH, 'scenario identity')
        require(local_file(source.parent, manifest['checkpointCapture'] + '/capture-complete.json') == capture_path, 'scenario capture differs from package')
        require(type(manifest['previousGrandpaSet']) is int and manifest['previousGrandpaSet'] == chain['previousGrandpaSetId'] and grandpa_rows(manifest['freshGrandpa'], hex_only=True) == grandpa_rows(chain['grandpaAuthorities']), 'scenario GRANDPA parameters differ from chain specification')

        def ref(value, executable=False, maximum=32 * 1024**2):
            keys(value, {'file', 'sha256'})
            path = local_file(source.parent, value['file'])
            role = add(path, value['sha256'], executable)
            require(records[role]['bytes'] <= maximum, 'referenced file size bound')
            return role

        expected_artifacts = {}
        for field, role, label in [('policy', 'policyScale', None), ('bootstrap', 'bootstrapWasm', 'bootstrap-134'), ('hold', 'holdWasm', 'hold-135'), ('finalRuntime', 'finalWasm', 'final-138')]:
            linked = ref(manifest[field])
            require(all(records[linked][key] == records[role][key] for key in ['sha256', 'blake2_256', 'bytes']), 'scenario runtime/policy differs from package')
            if label:
                expected_artifacts[label] = records[role]['sha256']
        steps = manifest['steps']
        require(isinstance(steps, list) and 2 <= len(steps) <= MAX_STEPS, 'scenario step bounds')
        details, dynamic = [], []
        prior_slot = 0
        prior_version = 134
        for index, step in enumerate(steps):
            require(isinstance(step, dict) and {'slot', 'timestamp', 'authorityIndex', 'expectedSpec', 'expectedPhase'} <= set(step)
                and set(step) <= {'slot', 'timestamp', 'authorityIndex', 'recovery', 'expectedSpec', 'expectedPhase', 'rejectPreviousEscapes', 'rejectedRecovery'}, 'fixture/unknown step fields rejected')
            require(uint(step['slot']) > prior_slot and uint(step['timestamp']) > 0 and uint(step['authorityIndex'], 2**32-1) < len(manifest['freshGrandpa']), 'step slot/time/authority bounds')
            require(index != 0 or step['slot'] == chain['firstRecoverySlot'], 'scenario first slot differs from chain specification')
            prior_slot = step['slot']
            require(type(step.get('rejectPreviousEscapes', False)) is bool and (scenario == 'hold' or not step.get('rejectPreviousEscapes', False)), 'unapproved escape-replay mode')
            negatives = step.get('rejectedRecovery', [])
            require(isinstance(negatives, list) and len(negatives) <= 128, 'negative envelope bound')
            negative_roles = [ref(value, maximum=MAX_ENVELOPE) for value in negatives]
            recovery_role = ref(step['recovery'], maximum=MAX_ENVELOPE) if step.get('recovery') is not None else None
            tag, operation_index = action(paths[recovery_role].read_bytes()) if recovery_role else (None, None)
            require(type(step['expectedSpec']) is int and isinstance(step['expectedPhase'], str), 'expected runtime/phase encoding')
            if scenario == 'activation':
                require(step['expectedSpec'] in (134, 138) and (prior_version != 138 or step['expectedSpec'] == 138)
                    and step['expectedPhase'] in ('Applying', 'Auditing', 'Ready', 'Complete')
                    and (recovery_role is not None) == (prior_version == 134) and tag != 4, 'normal activation path changed')
            else:
                require(step['expectedSpec'] == (134 if index == 0 else 135)
                    and step['expectedPhase'] == ('Applying' if index == 0 else 'Hold')
                    and tag == (4 if index == 1 else 0)
                    and (index > 1 or not step.get('rejectPreviousEscapes', False)),
                    'hold path requires first-child Tick, branch-bound second-child escape and subsequent hold execution')
            details.append({'step': step, 'recoveryRole': recovery_role, 'negativeRoles': negative_roles, 'action': tag, 'operationIndex': operation_index, 'executingSpec': prior_version})
            if tag in (6, 7):
                dynamic.append(index)
            prior_version = step['expectedSpec']
        native = None
        if 'nativeOperationRecorder' in manifest:
            config = manifest['nativeOperationRecorder']
            keys(config, {'executable', 'engineBinding'})
            executable_role = ref(config['executable'], executable=True, maximum=2 * 1024**3)
            binding_role = ref(config['engineBinding'], maximum=4 * 1024**2)
            binding = history.read(paths[binding_role])
            require(binding.get('schema') == 'sora-nexus-native-operation-engine-binding-v1' and binding.get('fixtureOnly') is False and binding.get('productionReady') is False
                and binding.get('engineSha256') == wasm['134']['sha256'] and binding.get('engineBlake2') == wasm['134']['blake2']
                and binding.get('policySha256') == records['policyScale']['sha256'] and binding.get('policyCommitment') == policy
                and binding.get('sdk') == 'polkadot-stable2603-3' and binding.get('specVersion') == 134 and binding.get('transactionVersion') == 131,
                'nonfixture native recorder binding differs')
            require(all(name in binding for name in ['runtimeFeatureProfile', 'hostLockAmendment', 'sdkCompilationRepair']), 'native build amendment references absent')
            native = {'executableRole': executable_role, 'bindingRole': binding_role, 'executableSha256': config['executable']['sha256'], 'bindingSha256': config['engineBinding']['sha256']}
            expected_artifacts.update({'native-operation-executable': native['executableSha256'], 'native-operation-engine-binding': native['bindingSha256']})
        require(bool(dynamic) == (native is not None), 'dynamic work requires its exact native recorder; unrelated recorder rejected')
        if scenario == 'activation':
            require(details[-1]['executingSpec'] == 138 and details[-1]['step']['expectedPhase'] == 'Complete', 'activation must execute a subsequent final138 block')
        else:
            supplied = manifest.get('suppliedCouncil')
            keys(supplied, {'escapes', 'expectedTerminal'})
            require(len(details) >= 3 and supplied['escapes'] == [{'step': 1, 'envelopeSha256': records[details[1]['recoveryRole']]['sha256'], 'destinationSpec': 135}], 'only the exact second-child hold authorization is admitted')
            terminal = supplied['expectedTerminal']
            keys(terminal, {'specVersion', 'phase', 'cursor', 'auditCursor', 'operationCounter', 'activePolicyCommitment', 'stateRoot', 'terminalAuditBlake2', 'ordinaryBlocks'})
            require(terminal['specVersion'] == 135 and terminal['phase'] == 'Hold' and all(type(terminal[name]) is int and terminal[name] == value for name, value in [('cursor', 0), ('auditCursor', 0), ('operationCounter', 1), ('ordinaryBlocks', 0)])
                and terminal['activePolicyCommitment'] == '0x' + policy and terminal['terminalAuditBlake2'] is None, 'hold terminal differs from unchanged original policy')
            digest_hex(terminal['stateRoot'], True)
        context['scenarios'][scenario] = {'manifestRole': manifest_role, 'manifestSha256': records[manifest_role]['sha256'], 'manifest': manifest, 'steps': details, 'native': native, 'dynamicSteps': dynamic, 'expectedArtifacts': expected_artifacts}
    for role in list(input_roles):
        add(paths[role])
    context['inputRoles'] = sorted(input_roles)
    return context


def block_header(raw):
    reader = encoding.Scale(raw)
    parent = '0x' + reader.take(32).hex()
    number = reader.integer()
    root = '0x' + reader.take(32).hex()
    reader.take(32)  # Native author/import already verifies ordered extrinsics root.
    count = reader.integer()
    require(count <= 1024, 'block digest count')
    logs = []
    for _ in range(count):
        tag = reader.take(1)[0]
        if tag in (4, 5, 6):
            logs.append((tag, reader.take(4), reader.vector()))
        elif tag == 0:
            logs.append((tag, b'', reader.vector()))
        else:
            require(tag == 8, 'unknown block digest')
            logs.append((tag, b'', b''))
    digest = '0x' + blake(raw[:reader.pos])
    count = reader.integer()
    require(1 <= count <= 2, 'unexpected authored extrinsic count')
    extrinsics = [reader.vector() for _ in range(count)]
    require(reader.pos == len(raw), 'trailing authored block bytes')
    return {'number': number, 'parentHash': parent, 'stateRoot': root, 'hash': digest, 'logs': logs, 'extrinsics': extrinsics}


class OutputEvidence:
    """Hash-bound file and snapshot closure shared by independent replay gates."""
    def __init__(self, output, pins):
        self.output, self.pins = output, pins

    def pinned(self, path, expected=None, maximum=MAX_REPORT):
        import production_build as process_tools
        path = Path(path)
        require(str(path) in self.pins, 'unbound generated/input file')
        pin = self.pins[str(path)]
        if expected is not None:
            digest_hex(expected)
            require(pin['sha256'] == expected, 'generated file pin differs')
        return process_tools.read(path, pin, maximum)

    def local(self, name):
        from release_package import local_file
        return local_file(self.output, name)

    def document(self, name, expected=None):
        return history.parse(self.pinned(self.local(name), expected))

    def object_ref(self, value, prefix=''):
        keys(value, {'file', 'sha256'})
        digest_hex(value['sha256'])
        require(value['file'] == 'snapshot-objects/' + value['sha256'] + '.scale.gz', 'snapshot object path')
        path = self.local(prefix + value['file'])
        require(str(path) in self.pins and self.pins[str(path)]['sha256'] == value['sha256'], 'snapshot object is not pinned')

    def journal(self, rows):
        names = {str(path.relative_to(self.output)) for path in self.output.glob('snapshots/*.json')}
        require(names == {f'snapshots/{index:06d}.json' for index in range(len(rows) + 1)}, 'fresh snapshot journal has gaps or extras')
        parent, base = None, None
        for index in range(len(rows) + 1):
            name = f'snapshots/{index:06d}.json'
            expected = rows[index - 1]['diskSnapshotManifestSha256'] if index else None
            value = self.document(name, expected)
            keys(value, {'schema', 'index', 'parent', 'base', 'delta', 'root', 'rawSnapshotSha256'})
            require(value['schema'] == 'sora-nexus-maintenance-snapshot-v1' and type(value['index']) is int and value['index'] == index
                and value['root'] == (history.ROOT if index == 0 else rows[index - 1]['stateRoot']), 'snapshot index/root differs')
            digest_hex(value['rawSnapshotSha256'])
            self.object_ref(value['base'])
            if index == 0:
                require(value['parent'] is None and value['delta'] is None, 'invalid initial snapshot')
                base = value['base']
            else:
                require(value['parent'] == parent and value['base'] == base, 'snapshot ancestry differs')
                self.object_ref(value['delta'])
            parent = {'file': name, 'sha256': self.pins[str(self.local(name))]['sha256']}

    def native_snapshot(self, prefix, report, authored, root):
        terminal = self.document(prefix + 'snapshots/000000.json', report['terminalSnapshotSha256'])
        keys(terminal, {'schema', 'index', 'parent', 'base', 'delta', 'root', 'rawSnapshotSha256'})
        require(terminal['schema'] == 'sora-nexus-maintenance-snapshot-v1' and type(terminal['index']) is int and terminal['index'] == 0
            and terminal['parent'] is None and terminal['delta'] is None and terminal['root'] == root, 'native restored snapshot context differs')
        digest_hex(terminal['rawSnapshotSha256'])
        self.object_ref(terminal['base'], prefix)
        require(self.pinned(self.local(prefix + 'block.scale'), maximum=2 * 1024**2) == authored
            and self.pinned(self.local(prefix + 'original-extrinsic.scale'), maximum=MAX_ENVELOPE + 1024)
                == scale.vec(block_header(authored)['extrinsics'][1]), 'native authored block or original extrinsic differs')


def validate(result, context, scenario, output, pins):
    """Join a fresh native report to the pinned files it actually generated.

    `inputPaths` is supplied internally from the owned input copy, never admitted
    from candidate JSON. Native replay has already reconstructed every snapshot.
    """
    import production_build as process_tools
    plan = context['scenarios'][scenario]
    mandatory = {'schema', 'manifestSha256', 'captureManifestSha256', 'policyCommitment', 'artifactSha256', 'blocks',
        'automaticActivationObserved', 'councilHoldObserved', 'forwardRepairObserved', 'ordinaryRuntimeBlocks',
        'livenessGraceObserved', 'feeClockShiftObserved', 'bridgeHaltRetained', 'nativeExecution',
        'consensusSealsVerified', 'operatorApprovalVerified', 'productionReady', 'publicTestCouncilFixture',
        'publicTestHashiFixture', 'fixtureGovernanceStateUnchanged', 'fixtureStateExpectationChecks',
        'nativeOperationDerivations', 'repairedNativeOperationDerivations', 'replayTranscriptSha256'}
    keys(result, mandatory | ({'suppliedCouncil'} if scenario == 'hold' else set())
         | ({'activationValidation'} if 'activationValidation' in result else set()))
    require(result['schema'] == ('sora-nexus-maintenance-replay-witness-v1' if scenario == 'activation' else 'sora-nexus-supplied-council-maintenance-replay-witness-v1'), 'fresh native report schema')
    require(result['manifestSha256'] == plan['manifestSha256'] and result['captureManifestSha256'] == context['captureManifestSha256']
        and result['policyCommitment'] == context['policyCommitment'] and result['artifactSha256'] == plan['expectedArtifacts'], 'fresh report/package artifact identities differ')
    require(all(result[key] is False for key in ['consensusSealsVerified', 'operatorApprovalVerified', 'productionReady', 'publicTestCouncilFixture', 'publicTestHashiFixture', 'forwardRepairObserved']), 'report asserts an unsupported scope')
    require(result['bridgeHaltRetained'] is True and result['fixtureGovernanceStateUnchanged'] is None
        and type(result['fixtureStateExpectationChecks']) is int and result['fixtureStateExpectationChecks'] == 0
        and result['repairedNativeOperationDerivations'] == [], 'fixture or repaired evidence rejected')
    require(type(result['livenessGraceObserved']) is bool and type(result['feeClockShiftObserved']) is bool, 'runtime flag encoding')
    require(result['automaticActivationObserved'] is (scenario == 'activation') and result['councilHoldObserved'] is (scenario == 'hold'), 'scenario transition missing')

    evidence = OutputEvidence(output, pins)
    pinned, local, document, object_ref = evidence.pinned, evidence.local, evidence.document, evidence.object_ref
    rows = result['blocks']
    require(isinstance(rows, list) and len(rows) == len(plan['steps']), 'report block count')
    evidence.journal(rows)

    transcript = sha(b'sora-nexus-maintenance-transcript-v1' + plan['manifestSha256'].encode() + context['captureManifestSha256'].encode())
    parent = scale.CHECKPOINT_HASH
    lineage = scale.MAGIC + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + bytes.fromhex(context['policyCommitment'])
    seen_versions = set()
    prior_transcripts = {}
    for index, (row, detail) in enumerate(zip(rows, plan['steps'])):
        keys(row, BLOCK_FIELDS)
        step = detail['step']
        version = detail['executingSpec']
        number = scale.CHECKPOINT + index + 1
        require(type(row['number']) is int and row['number'] == number and row['executingCodeBlake2'] == context['wasm'][str(version)]['blake2'], 'actual executing runtime differs from package')
        require(type(row['resultingSpec']) is int and row['resultingSpec'] == step['expectedSpec'] and row['phase'] == step['expectedPhase'], 'actual runtime/phase differs from manifest')
        require(row['independentImportRootMatches'] is True and row['diskRestartRootMatches'] is True
            and row['maintenanceSessionFrozen'] is (version != 138) and row['generatedAuthorizationSha256'] is None, 'missing Wasm import/restart/frozen evidence or fixture authorization')
        require(uint(row['cursor'], context['workChunks']) >= 0 and uint(row['operationCounter']) == (1 if scenario == 'hold' and index >= 1 else 0), 'unexpected cursor/escape counter')
        expected_rejected = (2 + len(detail['negativeRoles']) + (1 if scenario == 'hold' and index > 1 else 0)) if version != 138 else 0
        require(type(row['forbiddenExtrinsicsRejected']) is int and row['forbiddenExtrinsicsRejected'] == expected_rejected, 'native negative-check count differs')
        raw = pinned(local(f'{number}.block.scale'), row['blockSha256'], 2 * 1024**2)
        header = block_header(raw)
        if scenario == 'hold' and index == 0:
            require(not any(tag == 5 for tag, _, _ in header['logs']),
                    'rehearsal branch anchor must be an unsealed child, distinct from any valid production BABE block')
        require(header['number'] == number and header['parentHash'] == parent and header['hash'] == row['hash'] and header['stateRoot'] == row['stateRoot'], 'actual block ancestry/hash/state witness differs')
        require([data for tag, engine, data in header['logs'] if tag == 6 and engine == b'BABE'] == [b'\x02' + scale.u32(step['authorityIndex']) + step['slot'].to_bytes(8, 'little')], 'block BABE input differs')
        require([data for tag, _, data in header['logs'] if tag == 0 and data.startswith(scale.MAGIC)] == [lineage], 'actual block policy lineage differs')
        expected_extrinsics = [b'\x04\x01\x00' + scale.compact(step['timestamp'])]
        if detail['recoveryRole'] is not None:
            envelope = pinned(Path(context['inputPaths'][detail['recoveryRole']]), maximum=MAX_ENVELOPE)
            expected_extrinsics.append(b'\x04\x00\x00' + scale.vec(envelope))
        require(header['extrinsics'] == expected_extrinsics, 'authored envelope/time bytes differ from committed input')
        prior_transcripts[number] = transcript
        encoded = json.dumps({key: row[key] for key in BLOCK_FIELDS}, separators=(',', ':'), ensure_ascii=False).encode()
        transcript = sha(transcript.encode() + encoded)
        parent = row['hash']
        seen_versions.add(version)
    require(transcript == result['replayTranscriptSha256'], 'fresh replay transcript differs')
    ordinary = sum(detail['executingSpec'] == 138 for detail in plan['steps'])
    require(type(result['ordinaryRuntimeBlocks']) is int and result['ordinaryRuntimeBlocks'] == ordinary, 'ordinary block count differs')
    require(seen_versions == ({134, 138} if scenario == 'activation' else {134, 135}), 'runtime installed but never executed')
    if scenario == 'activation':
        require(ordinary > 0 and rows[-1]['resultingSpec'] == 138 and rows[-1]['phase'] == 'Complete' and rows[-1]['cursor'] == context['workChunks'] and result['feeClockShiftObserved'] is True, 'normal activation did not finish')
    else:
        require(ordinary == 0 and all(row['cursor'] == 0 for row in rows) and result['feeClockShiftObserved'] is False and result['livenessGraceObserved'] is False, 'hold progressed ordinary economics')
        supplied = result['suppliedCouncil']
        keys(supplied, {'schema', 'authorizations', 'terminal', 'signaturesVerifiedAgainstPinnedPolicy', 'automaticActivationObserved', 'publicTestCouncilGenerated', 'councilInventoryApprovalVerified', 'sourceProvenanceVerified', 'productionReady', 'publicDynamicFixture'})
        require(supplied['schema'] == 'sora-nexus-supplied-council-replay-witness-v1' and supplied['signaturesVerifiedAgainstPinnedPolicy'] is True
            and all(supplied[key] is False for key in ['automaticActivationObserved', 'publicTestCouncilGenerated', 'councilInventoryApprovalVerified', 'sourceProvenanceVerified', 'productionReady', 'publicDynamicFixture']), 'hold quorum or scope evidence differs')
        require(supplied['terminal'] == plan['manifest']['suppliedCouncil']['expectedTerminal'] and supplied['terminal']['stateRoot'] == rows[-1]['stateRoot'], 'hold terminal differs from actual restored state')
        require(isinstance(supplied['authorizations'], list) and len(supplied['authorizations']) == 1, 'one supplied hold authorization required')
        witness = supplied['authorizations'][0]
        keys(witness, {'step', 'blockNumber', 'parentHash', 'recoveryAnchor', 'envelopeSha256', 'authorizationSha256', 'signingMessageSha256', 'phase', 'cursor', 'operationCounter', 'storedCodeBlake2', 'executingCodeBlake2', 'destinationCodeBlake2', 'destinationSpec', 'revisedPolicyCommitment', 'signers'})
        require(all(type(witness[name]) is int and witness[name] == value for name, value in [('step', 1), ('blockNumber', scale.CHECKPOINT + 2), ('cursor', 0), ('operationCounter', 0), ('destinationSpec', 135)])
            and witness['parentHash'] == rows[0]['hash'] and witness['recoveryAnchor'] == rows[0]['hash'] and witness['phase'] == 'Applying'
            and witness['storedCodeBlake2'] == '0x' + context['checkpointCodeBlake2']
            and witness['executingCodeBlake2'] == '0x' + context['wasm']['134']['blake2']
            and witness['destinationCodeBlake2'] == '0x' + context['wasm']['135']['blake2']
            and witness['revisedPolicyCommitment'] == '0x' + '00' * 32, 'hold authorization code/counter context differs')
        signers = witness['signers']
        require(isinstance(signers, list) and all(isinstance(s, str) and s in ['0x' + key for key in context['council']] for s in signers)
            and len(set(signers)) == len(signers) and len(signers) > len(context['council']) // 2, 'hold signer membership/quorum differs')
        prefix = f'{scale.CHECKPOINT + 2}.supplied.'
        require(document(prefix + 'authorization.json') == witness, 'authorization witness output differs')
        for suffix, field in [('escape.scale', 'envelopeSha256'), ('authorization.scale', 'authorizationSha256'), ('signing-message.scale', 'signingMessageSha256')]:
            pinned(local(prefix + suffix), witness[field], MAX_ENVELOPE)
        require(witness['envelopeSha256'] == plan['manifest']['suppliedCouncil']['escapes'][0]['envelopeSha256'], 'hold envelope differs from supplied authorization')

    derivations = result['nativeOperationDerivations']
    require(isinstance(derivations, list) and len(derivations) == len(plan['dynamicSteps']) and result['nativeExecution'] is bool(derivations), 'auxiliary native recorder scope differs')
    for derived, index in zip(derivations, plan['dynamicSteps']):
        number = scale.CHECKPOINT + index + 1
        native = plan['native']
        require(derived.get('block') == number and derived.get('operationIndex') == plan['steps'][index]['operationIndex']
            and derived.get('executableSha256') == native['executableSha256'] and derived.get('engineBindingSha256') == native['bindingSha256']
            and derived.get('parentReplayTranscriptSha256') == prior_transcripts[number], 'native derivation is detached from exact replay')
        require(all(derived.get(key) is True for key in ['joinedToVerifiedReplayPrefix', 'exactAuthoredBlockBytesMatch', 'completeOperationDeltaCompared', 'nativeTerminalSnapshotIndependentlyRestored'])
            and all(derived.get(key) is False for key in ['nativeSourceBuildAuthenticityVerified', 'sourceFinalityVerified', 'economicPolicyIndependentlyVerified', 'fixtureOnly', 'productionReady']), 'native derivation scope differs')
        require(derived.get('reportFile') == f'native-operation-{number}/recorded/report.json' and derived.get('processReceiptFile') == f'native-operation-{number}/process.json', 'native output location differs')
        native_report = document(derived['reportFile'], derived['reportSha256'])
        receipt = document(derived['processReceiptFile'], derived['processReceiptSha256'])
        require(native_report.get('blockSha256') == rows[index]['blockSha256'] and native_report.get('stateRoot') == rows[index]['stateRoot']
            and native_report.get('engineSha256') == context['wasm']['134']['sha256'] and native_report.get('policyCommitment') == '0x' + context['policyCommitment'], 'native report code/block/state differs')
        recorded = f'native-operation-{number}/recorded/'
        authored = pinned(local(f'{number}.block.scale'), rows[index]['blockSha256'], 2 * 1024**2)
        evidence.native_snapshot(recorded, native_report, authored, rows[index]['stateRoot'])
        require(receipt.get('schema') == 'sora-nexus-native-operation-process-v1' and type(receipt.get('pid')) is int and receipt['pid'] > 0
            and receipt.get('exitCode') == 0 and receipt.get('success') is True and receipt.get('stopReason') is None
            and receipt.get('executableSha256') == native['executableSha256'] and receipt.get('executable') == context['inputPaths'][native['executableRole']]
            and receipt.get('sourceFinalityVerified') is False and receipt.get('productionReady') is False, 'native auxiliary process receipt differs')
        for filename, field in [('stdout.json', 'stdoutSha256'), ('stderr.log', 'stderrSha256'), ('manifest.json', 'manifestSha256')]:
            pinned(local(f'native-operation-{number}/{filename}'), receipt[field])
        require(document(f'native-operation-{number}/stdout.json') == native_report, 'native stdout differs from retained report')


def run_owned(report, paths, binary, expected_sha256, parent, local_file, digest_file, profile):
    """Owned execution/retention only; trusted source wrappers supply fixed commands and validators."""
    context = report.get(profile['context'])
    if context is None:
        return
    import production_build as process_tools
    import release_package as package
    parent.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='.sora-nexus-' + profile['label'] + '-', dir=parent)).resolve()
    report[profile['directoryField']] = str(directory)
    original = {row['role']: row for row in report['artifacts']}
    pins = {}
    owned_paths = {}

    def retain(path, expected):
        require(str(path) not in pins or pins[str(path)] == expected, 'attempt to rebind accepted evidence')
        pins[str(path)] = expected

    def keep(path, value):
        pin = process_tools.write(path, value)
        retain(path, pin)
        return pin

    def copy(source, target, expected, executable=False):
        require(process_tools.digest(source) == expected, 'source changed before owned copy')
        retain(source, expected)
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            with source.open('rb') as original_file, target.open('xb') as copied_file:
                shutil.copyfileobj(original_file, copied_file, 1024 * 1024)
            target.chmod(0o555 if executable else 0o444)
        require(process_tools.digest(target) == expected and process_tools.digest(source) == expected, 'owned graph copy differs')
        require(target.stat().st_mode & 0o777 == (0o555 if executable else 0o444), 'owned input mode differs')
        retain(target, expected)

    for role in context['inputRoles']:
        record = original[role]
        source = paths[role]
        expected = {key: record[key] for key in ['sha256', 'blake2_256', 'bytes']}
        target = directory / 'inputs' / record['sourcePath']
        copy(source, target, expected, record['executable'])
        owned_paths[role] = str(target)
    helpers = [Path(__file__).resolve(), Path(package.__file__).resolve(), Path(history.__file__).resolve(),
        Path(encoding.__file__).resolve(), Path(scale.__file__).resolve(), Path(process_tools.__file__).resolve(),
        Path(process_tools.bootstrap.__file__).resolve(), Path(process_tools.build_helpers.__file__).resolve(),
        Path(package.incoming_replay_release.__file__).resolve(), *profile['sources']]
    for index, source in enumerate(helpers):
        expected = digest_file(source)
        retain(source, expected)
        target = directory / 'sources' / f'{index:02d}-{source.name}'
        keep(target, process_tools.read(source, expected))
        target.chmod(0o444)
    digest_hex(expected_sha256)
    verifier_pin = process_tools.digest(binary)
    require(verifier_pin['sha256'] == expected_sha256, 'explicit native verifier digest differs')
    verifier = directory / 'verifier'
    copy(binary, verifier, verifier_pin, True)
    keep(directory / 'input-pins.json', {'files': dict(pins), 'productionReady': False})
    generated = {}

    def guard():
        process_tools.guard_files(pins)
        require(verifier.stat().st_mode & 0o777 == 0o555 and verifier.stat().st_nlink == 1, 'owned verifier mode/link changed')

    verification = {**context, 'inputPaths': owned_paths}
    try:
        preparation = None
        if 'prepare' in profile:
            preparation = profile['prepare'](verification, verifier, directory, guard, keep, retain, pins)
            guard()
        for scenario in profile['scenarios']:
            plan = context['scenarios'][scenario]
            output = directory / scenario
            command = profile['command'](verifier, owned_paths, plan, output)
            raw, receipt = process_tools.process(command, directory, output, guard, timeout=REPLAY_TIMEOUT, maximum=MAX_REPORT, retain=retain)
            # Freeze every generated file before using any report. All accepted
            # pins remain guarded while the second scenario and packaging run.
            entries = list(output.rglob('*'))
            require(len(entries) <= MAX_FILES and all(not path.is_symlink() and path.resolve(strict=True) == path and (path.is_dir() or path.is_file()) for path in entries), 'generated output graph bound/alias')
            aggregate = 0
            for path in entries:
                if path.is_file():
                    pin = process_tools.digest(path)
                    aggregate += pin['bytes']
                    require(aggregate <= MAX_GRAPH, 'generated output size bound')
                    retain(path, pin)
            result = history.parse(raw)
            profile['validate'](result, verification, scenario, output / 'replay', pins)
            result = profile['select'](result)
            guard()
            generated[scenario] = {'manifestSha256': plan['manifestSha256'], 'executionReceiptSha256': digest_file(output / 'execution.json')['sha256'],
                'stdoutSha256': receipt['outputs']['stdout.log']['sha256'], 'replayTranscriptSha256': result['replayTranscriptSha256'], 'blocks': len(result['blocks'])}
        guard()
        keep(directory / 'verification.json', {'schema': profile['schema'], 'scenarios': generated,
            'wasmAuthorImportVerifiedByFreshNativeReplay': True, 'requiredExecutingVersions': profile['versions'](context),
            'consensusSealsVerified': False, 'operatorApprovalVerified': False, 'sourceBuildAuthenticityVerified': False, 'productionReady': False,
            **({'preparation': preparation} if preparation is not None else {})})
        entries = list(directory.rglob('*'))
        require(len(entries) <= 2 * MAX_FILES and all(not path.is_symlink() and path.resolve(strict=True) == path for path in entries), 'output graph alias/bound')
        files = sorted(path for path in entries if path.is_file())
        require({str(path) for path in files} == {name for name in pins if Path(name).is_relative_to(directory)}, 'unbound extra generated file')
        additions, added_paths, aggregate = [], {}, 0
        for index, path in enumerate(files):
            actual = process_tools.digest(path)
            require(actual == pins[str(path)], 'artifact changed before output registration')
            aggregate += actual['bytes']
            require(aggregate <= 3 * MAX_GRAPH and len(report['artifacts']) + len(files) <= 131072, 'package output graph bound')
            role = profile['outputRole'] + f'-{index:06d}'
            require(role not in paths, 'reserved output role collision')
            additions.append({'role': role, **actual, 'object': 'objects/' + actual['sha256'],
                'sourcePath': 'generated-' + profile['label'] + '/' + str(path.relative_to(directory)), 'executable': bool(path.stat().st_mode & 0o111)})
            added_paths[role] = path
        gate = profile['gate_result'](context, generated, preparation) if 'gate_result' in profile else {'status': 'verified', 'reason': profile['reason']}
        gate = {**gate, 'verifierSha256': expected_sha256, 'scenarios': generated,
            'verificationSha256': pins[str(directory / 'verification.json')]['sha256']}
        guard()
        report['artifacts'].extend(additions)
        paths.update(added_paths)
        report['gates'][profile['gate']] = gate
        report['productionReady'] = False
    except BaseException as error:
        process_tools.write(directory / 'failure.json', {'schema': profile['failureSchema'], 'error': str(error), 'completedScenarios': generated, 'productionReady': False})
        raise


def run(report, paths, binary, expected_sha256, parent, local_file, digest_file):
    import activation_post_state_release as activation
    observed = {}
    def checked(result, context, scenario, output, pins):
        validate(result, context, scenario, output, pins)
        if scenario == 'activation':
            observed['activation'] = activation.validate(result, context)
    profile = {'context': 'artifactIdentityReplay', 'gate': GATE, 'label': 'artifact-identity',
        'schema': 'sora-nexus-executing-artifact-identity-v1', 'outputRole': 'artifactIdentityOutput',
        'directoryField': 'artifactIdentityOutputDirectory', 'failureSchema': 'sora-nexus-executing-artifact-identity-failure-v1',
        'scenarios': ('activation', 'hold'), 'sources': [Path(activation.__file__).resolve()],
        'command': lambda verifier, owned, plan, output: [str(verifier), 'rehearse-maintenance', '--manifest', owned[plan['manifestRole']], '--output', str(output / 'replay')],
        'validate': checked, 'select': lambda result: result, 'versions': lambda context: [134, 135, 138],
        'reason': 'Fresh native replay executed the package exact134/135/138 Wasm, verified actual Core_version/transaction versions, independent import and persisted-state restoration; consensus/build/operator approval remain separate'}
    run_owned(report, paths, binary, expected_sha256, parent, local_file, digest_file, profile)
    # Registration happens only after the fresh process and its entire closure
    # pass the final guard. No candidate report can enter this path.
    if observed:
        parent_gate = report['gates'][GATE]
        report['gates'][activation.GATE] = {**observed['activation'],
            **{key: parent_gate[key] for key in ('verificationSha256', 'verifierSha256', 'scenarios')}}
