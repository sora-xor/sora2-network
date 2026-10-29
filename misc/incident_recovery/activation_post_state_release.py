"""Join the fresh exact-Wasm activation observer to its authenticated replay.

Only artifact_identity_release.run calls this after executing and validating its
owned process. An imported witness or success flag is never a release input.
"""
GATE = 'finalRuntimeActivationPostState'
SCHEMA = 'sora-nexus-exact-post-maintenance-activation-v1'


def validate(result, context):
    from artifact_identity_release import require, digest_hex
    require(result.get('publicTestCouncilFixture') is False
            and result.get('publicTestHashiFixture') is False
            and result.get('forwardRepairObserved') is False
            and result.get('publicSignedTransactions') is None,
            'activation gate requires the real normal recovery policy')
    witness = result.get('activationValidation')
    if witness is None:
        return {'status': 'unverified', 'reason': 'Fresh replay lacks the first-runtime migration observer'}
    require(isinstance(witness, dict) and witness.get('schema') == SCHEMA,
            'activation migration observer schema differs')
    require(witness.get('policyCommitment') == context['policyCommitment']
            and witness.get('executingCodeBlake2') == context['wasm']['138']['blake2'],
            'activation policy or exact runtime differs')
    details = context['scenarios']['activation']['steps']
    indices = [i for i, detail in enumerate(details) if detail['executingSpec'] == 138]
    require(indices and indices == list(range(indices[0], len(details))) and indices[0] > 0,
            'activation does not follow completed maintenance')
    rows = result['blocks']; first = indices[0]; parent = rows[first - 1]
    require(len(rows) == len(details) and parent['phase'] == 'Complete'
            and parent['resultingSpec'] == 138
            and witness.get('firstBlock') == rows[first]['number']
            and type(witness['firstBlock']) is int,
            'activation first block or maintenance phase differs')
    for field, source in [('maintenanceParentHash', 'hash'), ('maintenanceStateRoot', 'stateRoot'),
                          ('maintenanceSnapshotSha256', 'diskSnapshotManifestSha256')]:
        require(witness.get(field) == parent[source], 'activation parent detached: ' + field)
    digest_hex(witness.get('initializedStateRoot'), True)
    for field, expected in [('previousStakingVersion', 16), ('activatedStakingVersion', 18),
                            ('ordinaryBlocks', len(indices))]:
        require(type(witness.get(field)) is int and witness[field] == expected,
                'activation transition differs: ' + field)
    require(result['ordinaryRuntimeBlocks'] == len(indices)
            and witness.get('lastBlockHash') == rows[-1]['hash']
            and witness.get('lastStateRoot') == rows[-1]['stateRoot'],
            'activation continuation detached from executed blocks')
    for field in ('cutoffEra', 'firstExecutionEra', 'weightedPointsStartEra'):
        require(type(witness.get(field)) is int and 0 <= witness[field] < 2**32,
                'invalid activation era marker')
    require(witness['weightedPointsStartEra'] == min(witness['firstExecutionEra'] + 1, 2**32-1),
            'activation incentive start differs')
    require(witness.get('runtimeUpgradeExecutedAsWasm') is True
            and witness.get('independentImportAndRestartVerified') is True
            and all(witness.get(field) is False for field in
                    ('tryRuntimeFeatureChecksExecuted', 'fullCustomStakingVerified', 'productionReady')),
            'activation execution evidence or scope differs')
    for index in indices:
        row = rows[index]
        require(row['executingCodeBlake2'] == context['wasm']['138']['blake2']
                and row['resultingSpec'] == 138 and row['phase'] == 'Complete'
                and row['independentImportRootMatches'] is True
                and row['diskRestartRootMatches'] is True,
                'activation continuation did not execute/import/restore exact runtime')
    return {'status': 'verified' if len(indices) >= 2 else 'unverified',
            'reason': ('Exact production Wasm executed its migration on the completed maintenance state and at least two ordinary blocks, with independent import and disk restoration'
                       if len(indices) >= 2 else 'First runtime migration passed; a second ordinary block is still required'),
            'evidence': witness, 'productionReady': False}
