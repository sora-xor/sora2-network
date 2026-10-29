"""Fresh composition and final-state evidence; complete storage gate stays open.

The missing historical-derived discovery proof is never replaced by a supplied
inventory, report flag, archived replay, or a hash of such an assertion.
"""
from pathlib import Path
import copy
import artifact_identity_release as identity
import history_release as history
import financial_semantics_release as financial
import account_ledger_release as ledger
import position_conservation_release as conservation
import maintenance_observer_release as observer
import bridge_state_release as bridge

ROLE = 'storageCompositionInputs'
GATE = 'completeStorageSemantics'
JOINS = {'csv': 'signedMappingCsv', 'proofs': 'ownershipProofs', 'semantic': 'semanticPlan',
    'bridge-signing': 'bridgeSigningReview', 'bridge-hashi': 'nativeHashiReview',
    'staking-roles': 'stakingRoleReview'}
REMAINING = [
    'Complete mapped-account control history and derived-account discovery are not established by the supplied graph; absent intermediate derivatives and completed multisigs need authenticated historical calls/metadata.',
    'Final metadata unknown/legacy/hash-only storage and retained historical typed references need source-pinned dispositions; diagnostic absence is not mutable-ownership completeness.',
    'The committed financial equations are rechecked, but complete liability/beneficial-interest coverage and retired-key dispatch rejection remain separate unproved predicates.',
]


def scale_checkpoint_hash():
    return identity.scale.CHECKPOINT_HASH


def require(value, message):
    identity.require(value, 'storage semantics: ' + message)


def bind(root, records, paths, artifact_context, local_file, digest_file):
    if ROLE not in paths:
        return None
    import production_build as build
    import release_package as package
    require(artifact_context is not None, 'exact nonfixture activation/hold artifact bindings required')
    require(not any(role.startswith('repairWasm') for role in paths), 'repair policies need a separate storage revalidation corridor')
    source = paths[ROLE]
    value, graph, pin, commitment = build.load_inputs(source, records[ROLE]['sha256'])
    require(package.policy_identity(build.read(paths['policyScale']))['evidenceCommitment'] == commitment,
        'production input graph differs from policy data commitment')
    require(graph[value['compose']['capture'] + '/capture-complete.json'] == paths['checkpointCaptureManifest'], 'compose capture differs from package')
    for flag, role in JOINS.items():
        if flag in value['compose']:
            require(role in paths and graph[value['compose'][flag]] == paths[role], 'compose role differs: ' + role)
    require(graph[value['operatorsScale']] == paths['operatorsScale'] and graph[value['derivedInventory']] == paths['derivedAccountInventory'], 'compose operators/derived inventory differs')
    chain = history.read(paths['chainSpec'])['recovery']
    require(all(value['consensus'][field] == chain[field] for field in value['consensus']), 'compose consensus differs')
    indexed = {path: role for role, path in paths.items()}
    input_roles = set(artifact_context['inputRoles']) | {ROLE}
    graph_roles = {}
    for name, path in graph.items():
        actual = digest_file(path)
        require(actual == value['files'][name], 'bound composition file changed')
        if path not in indexed:
            role = f'storageCompositionInput-{len(indexed):05d}'
            require(role not in records and len(records) < 65536, 'reserved composition role/count bound')
            records[role] = {'role': role, **actual, 'object': 'objects/' + actual['sha256'],
                'sourcePath': path.relative_to(root).as_posix(), 'executable': False}
            paths[role] = path; indexed[path] = role
        role = indexed[path]
        require(all(records[role][key] == actual[key] for key in actual), 'composition artifact record changed')
        input_roles.add(role); graph_roles[name] = role
    # Identity.bind already closed the capture graph. Require that complete graph
    # to belong to this input commitment as well, not just its manifest.
    capture = paths['checkpointCaptureManifest'].parent
    for role in artifact_context['inputRoles']:
        if paths[role].is_relative_to(capture):
            require(paths[role] in graph.values(), 'capture page/artifact omitted from composition commitment')
    return {**copy.deepcopy(artifact_context), 'inputRoles': sorted(input_roles), 'composeManifestRole': ROLE,
        'composeManifestPin': pin, 'composeDataCommitment': commitment, 'composeGraphRoles': graph_roles,
        'terminalAuditCommitment': build.read(paths['policyScale'])[-32:].hex(),
        'completeStorageSemantics': False}


def compare_composition(context, composed, mixed, policy, read_owned):
    import release_package as package
    require(policy == read_owned('policyScale'), 'fresh native policy differs from package')
    if mixed is None:
        require(composed.get('schema') == 'sora-nexus-storage-work-v1', 'fresh composed schema')
        work, audits, terminal = composed['applyEnvelopes'], composed['auditEnvelopes'], composed['terminalEnvelope']
    else:
        work, audits, terminal = mixed['workEnvelopes'], mixed['auditEnvelopes'], mixed['terminalEnvelope']
    require(len(work) == context['workChunks'] and len(audits) == context['auditChunks'], 'fresh compiler chunk counts differ from bound policy')
    prelude = (composed if mixed is None else mixed).get('terminalPreludeEnvelopes', [])
    require(isinstance(prelude, list) and len(prelude) < 1024, 'terminal prelude bound')
    expected = [package.bounded_hex(value, identity.MAX_ENVELOPE) for value in [*work, *audits, *prelude, terminal]]
    actual = []
    for detail in context['scenarios']['activation']['steps']:
        role = detail['recoveryRole']
        if role is not None:
            raw = read_owned(role)
            tag, _ = identity.action(raw)
            if tag in (1, 2, 5, 6, 7, 8, 9, 10, 11):
                actual.append(raw)
            else:
                require(tag in (0, 3), 'unapproved activation action')
    require(actual == expected, 'activation work/audit/terminal bytes or order differ from fresh native compiler')
    return {'freshNativeCompositionVerified': True, 'policySha256': identity.sha(policy),
        'workEnvelopes': len(work), 'auditEnvelopes': len(audits),
        'orderedEnvelopesSha256': identity.sha(b''.join(len(raw).to_bytes(8,'little') + raw for raw in expected)),
        'terminalEnvelopeSha256': identity.sha(expected[-1]), 'completeStorageSemantics': False,
        'productionReady': False}


def prepare(context, verifier, directory, guard, keep, retain, pins):
    import production_build as build
    owned = {role: Path(path) for role, path in context['inputPaths'].items()}
    manifest = owned[context['composeManifestRole']]
    value, graph, pin, commitment = build.load_inputs(manifest, context['composeManifestPin'])
    require(pin == context['composeManifestPin'] and commitment == context['composeDataCommitment'], 'owned composition identity differs')
    require(all(graph[name] == owned[role] for name, role in context['composeGraphRoles'].items()), 'owned composition layout differs')
    out = directory / 'composition'
    out.mkdir()
    policy, _ = build.compose_inputs(value, manifest.parent, out, verifier, guard, commitment,
        remember=keep, retain=retain)
    guard()
    composed = build.parse(build.read(out / 'compose/stdout.log', pins[str(out / 'compose/stdout.log')], 512*1024**2))
    mixed = build.parse(build.read(out / 'mixed/stdout.log', pins[str(out / 'mixed/stdout.log')], 512*1024**2)) if value['mixedLayout'] is not None else None
    result = compare_composition(context, composed, mixed, policy,
        lambda role: build.read(owned[role], pins[str(owned[role])]))
    guard()
    return result


def validate(result, context, scenario, output, pins):
    require(scenario == 'activation', 'only normal activation is admitted')
    require(isinstance(result, dict) and 'postStateSemantics' in result, 'same-process native final-state checks are required')
    base = {key: value for key, value in result.items() if key != 'postStateSemantics'}
    identity.validate(base, context, scenario, output, pins)
    post = result['postStateSemantics']
    require(isinstance(post, dict) and post.get('schema') == 'sora-nexus-fresh-post-state-semantics-v1', 'final-state schema')
    last = result['blocks'][-1]
    require(post.get('captureManifestSha256') == context['captureManifestSha256'] and post.get('checkpointStateRoot') == history.ROOT
        and post.get('finalStateRoot') == last['stateRoot'] and post.get('finalBlockHash') == last['hash']
        and post.get('executingCodeBlake2') == context['wasm']['138']['blake2'] and post.get('policyCommitment') == context['policyCommitment']
        and post.get('terminalAuditCommitment') == context['terminalAuditCommitment'], 'same-process final state/package lineage differs')
    identity.OutputEvidence(output, pins).pinned(output / 'post-state-metadata.scale', post.get('finalMetadataSha256'), 32 * 1024**2)
    for key in ('finalMetadataSha256','terminalAuditCommitment'):
        identity.digest_hex(post.get(key))
    identity.digest_hex(post.get('finalMetadataBlake2'), True)
    prior = post.get('checkpointMetadataWitness')
    require(isinstance(prior, dict) and prior.get('checkpointHash') == scale_checkpoint_hash()
        and prior.get('stateRoot') == history.ROOT and prior.get('executingCodeBlake2') == '0x' + context['checkpointCodeBlake2']
        and prior.get('wasmExecutionVerified') is True and prior.get('nativeExecution') is False and prior.get('productionReady') is False, 'checkpoint metadata witness differs')
    for key in ('finalMetadataExecutedAsWasm','committedTerminalFinancialInvariantsRechecked','readOnlyStateRootMatches'):
        require(post.get(key) is True, 'native final check missing: ' + key)
    for key in ('finalMetadataNativeExecution','fullFinancialLiabilityCoverageVerified','retiredKeyDispatchRejectionVerified','completeStorageSemantics','productionReady'):
        require(post.get(key) is False, 'unsupported final-state scope: ' + key)
    for key in ('retirementAbsenceWitnessesRechecked','issuanceEquationsRechecked','collateralEquationsRechecked','zeroClaimsRechecked'):
        require(type(post.get(key)) is int and 0 <= post[key] <= 16384, 'native final check count')
    require(post['issuanceEquationsRechecked'] > 0, 'empty terminal finance proof')
    scan = post.get('scan')
    require(isinstance(scan, dict) and scan.get('completeTrieEnumerated') is True and type(scan.get('allEntriesTyped')) is bool
        and all(scan.get(key) is False for key in ('historicalReferencesDispositionVerified','derivedAccountDiscoveryComplete','completeMutableOwnershipVerified')), 'typed scan missing or overstated')
    for key in ('entries','bytes','childTries','metadataDecodedEntries','retiredTypedReferences'):
        require(type(scan.get(key)) is int and scan[key] >= 0, 'typed scan count')
    require(scan['entries'] <= 4_000_000 and scan['bytes'] <= 64*1024**3 and scan['childTries'] <= 4096, 'typed scan bound')
    identity.digest_hex(scan.get('typedScanSha256'))
    observer.validate(result, context, scenario)
    financial.validate(post.get('crossRecordFinancialEquations'))
    ledger.validate(post.get('accountLedgerEquations'), post)
    conservation.validate(post.get('positionConservation'), post, last, last['stateRoot'])
    bridge.validate(result, context, scenario)


def run(report, paths, binary, expected_sha256, parent, local_file, digest_file):
    observations = {}
    def select(result):
        observations['activation'] = observer.validate(result, report['storageSemanticsReplay'], 'activation')
        observations['gate'] = observer.supporting_gate(observations['activation'])
        observations['bridgeGate'] = bridge.supporting_gate(bridge.validate(result, report['storageSemanticsReplay'], 'activation'))
        return result
    profile = {'context': 'storageSemanticsReplay', 'gate': GATE, 'label': 'storage-semantics',
        'schema': 'sora-nexus-fresh-storage-semantics-evidence-v1', 'outputRole': 'storageSemanticsOutput',
        'directoryField': 'storageSemanticsOutputDirectory', 'failureSchema': 'sora-nexus-storage-semantics-failure-v1',
        'scenarios': ('activation',), 'sources': [Path(__file__).resolve(), Path(observer.__file__).resolve(), Path(financial.__file__).resolve(), Path(ledger.__file__).resolve(), Path(conservation.__file__).resolve(), Path(bridge.__file__).resolve()], 'prepare': prepare,
        'command': lambda verifier, owned, plan, output: [str(verifier), 'rehearse-storage-semantics', '--manifest', owned[plan['manifestRole']], '--output', str(output / 'replay')],
        'validate': validate, 'select': select, 'versions': lambda context: [134, 138],
        'gate_result': lambda context, scenarios, preparation: {'status': 'unverified',
            'reason': 'Fresh native composition, exact normal Wasm activation and final-state typed/committed financial checks passed; full storage semantics remains incomplete',
            'freshSupportingEvidenceVerified': True, 'remaining': REMAINING}}
    identity.run_owned(report, paths, binary, expected_sha256, parent, local_file, digest_file, profile)
    if observations:
        storage_gate = report['gates'][GATE]
        gate = {**observations['gate'], **{key: storage_gate[key] for key in ('verificationSha256', 'verifierSha256', 'scenarios')}}
        # Derived only from the already-registered fresh invocation; no file read
        # or new external admission occurs after the final guarded registration.
        bridge_gate = {**observations['bridgeGate'], **{key: storage_gate[key] for key in ('verificationSha256', 'verifierSha256', 'scenarios')}}
        report['gates'][observer.GATE] = gate
        report['gates'][bridge.GATE] = bridge_gate
