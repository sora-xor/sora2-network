"""Join the fresh exact-Wasm activation observer to its authenticated replay.

Only artifact_identity_release.run calls this after executing and validating its
owned process. An imported witness or success flag is never a release input.
"""
GATE = 'finalRuntimeActivationPostState'
SCHEMA = 'sora-nexus-exact-post-maintenance-activation-v2'


def validate(result, context, scenario='activation'):
    if scenario not in ('activation', 'repair'):
        raise ValueError('unknown activation scenario')
    from artifact_identity_release import require, digest_hex
    require(result.get('publicTestCouncilFixture') is False
            and result.get('publicTestHashiFixture') is False
            and result.get('forwardRepairObserved') is (scenario == 'repair')
            and result.get('publicSignedTransactions') is None,
            'activation gate requires the real recovery policy')
    witness = result.get('activationValidation')
    if witness is None:
        return {'status': 'unverified', 'reason': 'Fresh replay lacks the activation and staking-boundary observer'}
    if isinstance(witness, dict) and witness.get('schema') == 'sora-nexus-exact-post-maintenance-activation-v1':
        return {'status': 'unverified', 'reason': 'Legacy activation observer does not cover session and era transitions'}
    require(isinstance(witness, dict) and witness.get('schema') == SCHEMA,
            'activation migration observer schema differs')
    require(witness.get('lineagePolicyCommitment') == context['policyCommitment']
            and witness.get('policyCommitment') == (context['policies']['138']['commitment'] if scenario == 'repair' else context['policyCommitment'])
            and witness.get('executingCodeBlake2') == context['wasm']['138']['blake2'],
            'activation policy or exact runtime differs')
    details = context['scenarios'][scenario]['steps']
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
    boundaries = validate_boundaries(witness, rows, details, first)
    return {'status': 'verified' if boundaries else 'unverified',
            'reason': ('Exact production Wasm migrated the completed maintenance state, crossed the first session and active-era transitions, and executed a subsequent block with checked async staking state, independent import and disk restoration'
                       if boundaries else 'Activation must cross the first session and active-era transitions and execute a subsequent ordinary block after each'),
            'evidence': witness, 'productionReady': False}


def validate_boundaries(witness, blocks, details, first):
    """Join native state observations to actual authored blocks and input slots.

    These are checks of the pinned native observer's output, not a replacement
    for its typed storage checks or the separately built try-runtime audit.
    """
    from artifact_identity_release import require, digest_hex

    def uint(value, maximum=2**32-1):
        require(type(value) is int and 0 <= value <= maximum, 'invalid activation state integer')
        return value

    maintenance = witness.get('maintenance')
    require(isinstance(maintenance, dict), 'missing activation maintenance history')
    for field in ('checkpointEpoch', 'firstRecoveryEpoch', 'lastEpoch'):
        uint(maintenance.get(field), 2**64-1)
    for field in ('checkpointSession', 'checkpointActiveEra', 'checkpointCurrentEra', 'blocks'):
        uint(maintenance.get(field))
    for field in ('checkpointSlot', 'firstRecoverySlot', 'lastSlot', 'maximumSlotGap'):
        uint(maintenance.get(field), 2**64-1)
    slots = [maintenance['checkpointSlot']] + [row['step']['slot'] for row in details[:first]]
    require(all(type(slot) is int and 0 <= slot < 2**64 for slot in slots)
            and all(a < b for a, b in zip(slots, slots[1:]))
            and maintenance['firstRecoverySlot'] == slots[1]
            and maintenance['lastSlot'] == slots[-1]
            and maintenance['maximumSlotGap'] == max(b-a for a, b in zip(slots, slots[1:]))
            and maintenance['blocks'] == first
            and maintenance['checkpointCurrentEra'] == witness['cutoffEra']
            and maintenance['checkpointActiveEra'] == witness['firstExecutionEra']
            and maintenance['checkpointEpoch'] <= maintenance['firstRecoveryEpoch'] <= maintenance['lastEpoch']
            and maintenance['checkpointActiveEra'] <= maintenance['checkpointCurrentEra'] <= maintenance['checkpointActiveEra'] + 1,
            'activation maintenance slots, epochs or frozen eras differ')
    observed = witness.get('rows')
    require(isinstance(observed, list) and len(observed) == len(blocks) - first,
            'activation state observation count differs')
    prior = {'slot': slots[-1], 'epoch': maintenance['lastEpoch'],
             'session': maintenance['checkpointSession'], 'activeEra': maintenance['checkpointActiveEra'],
             'currentEra': maintenance['checkpointCurrentEra']}
    transitions = {'firstSession': None, 'firstActiveEra': None,
                   'postFirstSessionBlocks': 0, 'postFirstActiveEraBlocks': 0}
    for offset, row in enumerate(observed):
        index = first + offset; block = blocks[index]
        require(isinstance(row, dict), 'activation state row malformed')
        for field, source in [('block', 'number'), ('blockHash', 'hash'), ('stateRoot', 'stateRoot'),
                              ('snapshotManifestSha256', 'diskSnapshotManifestSha256'),
                              ('executingCodeBlake2', 'executingCodeBlake2')]:
            require(type(row.get(field)) is type(block[source]) and row[field] == block[source],
                    'activation state observation detached: ' + field)
        require(row.get('readOnlyStateRootMatches') is True, 'activation state observation changed storage')
        for field in ('slot', 'epoch', 'timestamp', 'activeEraStart'):
            uint(row.get(field), 2**64-1)
        for field in ('session', 'currentEra', 'activeEra'):
            uint(row.get(field))
        require(row['slot'] == details[index]['step']['slot'] and row['slot'] > prior['slot']
                and row['timestamp'] == details[index]['step']['timestamp']
                and row['activeEraStart'] <= row['timestamp']
                and row['epoch'] >= prior['epoch']
                and prior['session'] <= row['session'] <= prior['session'] + 1
                and prior['activeEra'] <= row['activeEra'] <= prior['activeEra'] + 1
                and prior['currentEra'] <= row['currentEra'] <= prior['currentEra'] + 1
                and (row['epoch'] > prior['epoch']) == (row['session'] > prior['session'])
                and row['activeEra'] <= row['currentEra'] <= row['activeEra'] + 1,
                'activation consensus/era progression differs')
        if row['session'] != prior['session']:
            require(row['epoch'] > prior['epoch'], 'session transition lacks an epoch transition')
        if row['activeEra'] != prior['activeEra']:
            require(row['session'] != prior['session'] and row['activeEraStart'] == row['timestamp'],
                    'active era advanced without session rotation or its timestamp baseline')
        state = row.get('asyncState')
        require(isinstance(state, dict) and set(state) == {'pendingValidators', 'pendingEra', 'pruneUpTo',
                'queuedSession', 'queuedEra', 'electionDeadline', 'forced', 'timestampEra'}
                and type(state.get('forced')) is bool,
                'activation async state malformed')
        for field in ('pendingEra', 'pruneUpTo', 'queuedSession', 'queuedEra', 'electionDeadline', 'timestampEra'):
            if state.get(field) is not None:
                uint(state[field])
        pending = uint(state.get('pendingValidators'), 1200)
        require(state.get('timestampEra') is None
                and ((state.get('pendingEra') is None) == (pending == 0))
                and ((state.get('queuedEra') is None) == (state.get('queuedSession') is None))
                and not (pending and state.get('queuedEra') is not None),
                'activation pending/queued state is inconsistent')
        if pending:
            require(state['pendingEra'] == row['currentEra'] == row['activeEra'] + 1,
                    'activation pending election era differs')
        if state.get('queuedEra') is not None:
            require(state['queuedEra'] == row['currentEra'] == row['activeEra'] + 1
                    and state['queuedSession'] == row['session'] + 1 and state['electionDeadline'] is None,
                    'activation queued election/session differs')
        recent = row.get('lastValidatorEra')
        require(isinstance(recent, dict), 'activation validator recency missing')
        count = uint(recent.get('count'), 100000); digest_hex(recent.get('sha256'))
        seen_eras = {}
        for name in ('active', 'queued', 'pending'):
            values = recent.get(name)
            require(isinstance(values, list) and len(values) <= min(count, 1200), 'activation validator set malformed')
            accounts = set()
            for value in values:
                require(isinstance(value, dict), 'activation validator recency row malformed')
                digest_hex(value.get('account'), True)
                require(value['account'] not in accounts, 'duplicate activation validator')
                accounts.add(value['account'])
                era = uint(value.get('era'))
                require(row['activeEra'] <= era <= row['currentEra'], 'activation validator recency differs')
                require(name == 'active' or era == state[name + 'Era'],
                        'activation pending/queued validator recency differs')
                require(value['account'] not in seen_eras or seen_eras[value['account']] == era,
                        'activation validator recency contradicts another set')
                seen_eras[value['account']] = era
            if name == 'active':
                require(bool(values), 'empty activation validator set')
            elif name == 'pending':
                require(len(values) == pending, 'pending validator recency coverage differs')
            else:
                require(bool(values) == (state.get('queuedEra') is not None), 'queued validator recency coverage differs')
        for field, transition in [('session', 'firstSession'), ('activeEra', 'firstActiveEra')]:
            if transitions[transition] is None and row[field] != prior[field]:
                transitions[transition] = {'block': row['block'], 'before': prior[field], 'after': row[field]}
            if transitions[transition] is not None and row['block'] > transitions[transition]['block']:
                transitions['post' + transition[0].upper() + transition[1:] + 'Blocks'] += 1
        prior = row
    require(witness.get('transitions') == transitions, 'activation transition summary differs from observed state')
    return all(transitions[name] is not None for name in ('firstSession', 'firstActiveEra')) and all(
        transitions[name] >= 1 for name in ('postFirstSessionBlocks', 'postFirstActiveEraBlocks'))
