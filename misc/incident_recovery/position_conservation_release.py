"""Native same-invocation owner-position contract; never an evidence admission API."""
import artifact_identity_release as identity

TRUE = ('allThreePrefixesEnumerated', 'replacementFinancialDestinationsUnused',
        'freeReservedAndLpOwnershipConserved', 'nativeIncomingMovementsOpaqueVerified')
FALSE = ('genericStorageEditAllowanceUsed', 'attackerDispositionsVerified',
         'debtAndCollateralConservationVerified', 'derivedOwnershipComplete',
         'nativeIncomingEconomicsIndependentlyVerified', 'accountLifecycleFieldsConserved',
         'completeFinancialCoverageVerified', 'completeStorageSemantics', 'productionReady')
FIELDS = {'schema', 'fixtureOnly', 'checkpointStateRoot', 'checkpointMetadataHash',
    'inspectionStateRoot', 'inspectionBlock', 'inspectionBlockHash', 'inspectionSnapshotManifestSha256',
    'finalMetadataHash', 'certificateTerminalStateRoot', 'policyCommitment', 'replacementCount',
    'checkpointRows', 'inspectionRows', 'checkpointBytes', 'inspectionBytes',
    'checkpointPositionSha256', 'expectedPositionSha256', 'inspectionPositionSha256',
    'incomingOperations', 'incomingDeltaRows', 'incomingJournalSha256', *TRUE, *FALSE}


def need(ok, why):
    identity.require(ok, 'owner position conservation: ' + why)


def validate(value, post, witness, terminal_root, fixture=False):
    need(isinstance(value, dict) and set(value) == FIELDS
        and value['schema'] == 'sora-nexus-owner-position-conservation-v1', 'exact native result required')
    need(type(fixture) is bool and value['fixtureOnly'] is fixture, 'fixture scope differs')
    need(value['checkpointStateRoot'] == post['checkpointStateRoot']
        and value['inspectionStateRoot'] == post['finalStateRoot'] == witness['stateRoot']
        and value['inspectionBlockHash'] == post['finalBlockHash'] == witness['hash']
        and type(value['inspectionBlock']) is int and value['inspectionBlock'] == witness['number']
        and value['inspectionSnapshotManifestSha256'] == witness['diskSnapshotManifestSha256']
        and value['finalMetadataHash'] == post['finalMetadataBlake2']
        and value['policyCommitment'] == post['policyCommitment']
        and value['certificateTerminalStateRoot'] == terminal_root,
        'actual checkpoint/inspection/opaque terminal identity differs')
    need(fixture or terminal_root == value['inspectionStateRoot'], 'normal inspection must be replay terminal')
    for key in ('checkpointStateRoot', 'checkpointMetadataHash', 'inspectionStateRoot',
                'inspectionBlockHash', 'finalMetadataHash', 'certificateTerminalStateRoot'):
        identity.digest_hex(value[key], True)
    for key in ('inspectionSnapshotManifestSha256', 'policyCommitment', 'checkpointPositionSha256',
                'expectedPositionSha256', 'inspectionPositionSha256', 'incomingJournalSha256'):
        identity.digest_hex(value[key])
    need(value['expectedPositionSha256'] == value['inspectionPositionSha256'], 'per-owner positions differ')
    for key in TRUE:
        need(value[key] is True, 'native equation missing: ' + key)
    for key in FALSE:
        need(value[key] is False, 'unsupported scope promoted: ' + key)
    for key, bound in (('replacementCount',4096),('incomingOperations',16384),('incomingDeltaRows',65536)):
        need(type(value[key]) is int and 0 <= value[key] <= bound, 'count bound: ' + key)
    need(value['incomingOperations'] > 0 or value['incomingDeltaRows'] == 0, 'movement without an opaque incoming operation')
    for which in ('checkpoint','inspection'):
        rows = value[which+'Rows']
        need(isinstance(rows,list) and len(rows)==3 and all(type(n) is int and 0<=n<=2_000_000 for n in rows)
            and sum(rows)<=2_000_000, 'complete family scan count bound')
        size=value[which+'Bytes']
        need(type(size) is int and 0<=size<=512*1024**2, 'complete family scan byte bound')
    return value
