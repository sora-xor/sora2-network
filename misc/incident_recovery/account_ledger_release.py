"""Narrow result contract, used only inside the existing owned fresh replay."""
import artifact_identity_release as identity

TRUE = ('nativeAccountSumEqualsIssuance', 'allTokenAccountSumsReconciled',
    'exactLegacyMarkerDerivationsVerified', 'legacyMarkerRecordsAndIssuanceUnchanged',
    'legacyMarkerAccountsRemainAbsent', 'currentMarkerPoolSharesEqualIssuance')
FALSE = ('historicalMigrationExecutionVerified', 'checkpointToFinalOwnershipConservationVerified',
    'completeFinancialCoverageVerified', 'completeStorageSemantics', 'productionReady')
COUNTS = ('checkpointNativeAccountRows', 'finalNativeAccountRows', 'checkpointTokenAccountRows',
    'finalTokenAccountRows', 'checkpointTokenIssuanceRows', 'finalTokenIssuanceRows', 'checkpointRows', 'finalRows')
FIELDS = {'schema', 'checkpointStateRoot', 'checkpointMetadataHash', 'finalMetadataHash',
    'unclassifiedOrphanIssuanceRows', 'legacyMarkerRows', 'legacyMarkerIdentitySha256',
    'checkpointNativeIssuance', 'finalNativeIssuance', 'checkpointBytes', 'finalBytes', *TRUE, *FALSE, *COUNTS}

def need(ok, message):
    identity.require(ok, 'account ledger equations: ' + message)

def decimal(value):
    need(isinstance(value, str) and value.isascii() and value.isdecimal()
        and str(int(value)) == value and int(value) < 2**128, 'noncanonical u128 amount')
    return int(value)

def validate(value, post):
    need(isinstance(value, dict) and set(value) == FIELDS
        and value['schema'] == 'sora-nexus-full-account-ledger-equations-v1', 'native result schema differs')
    need(value['checkpointStateRoot'] == post['checkpointStateRoot']
        and value['finalMetadataHash'] == post['finalMetadataBlake2'], 'same-invocation root/metadata identity differs')
    for key in ('checkpointMetadataHash', 'finalMetadataHash'):
        need(isinstance(value[key], str) and value[key].startswith('0x'), 'metadata hash prefix')
        identity.digest_hex(value[key][2:])
    for key in TRUE:
        need(value[key] is True, 'native equation missing: ' + key)
    for key in FALSE:
        need(value[key] is False, 'unsupported broad claim: ' + key)
    need(type(value['unclassifiedOrphanIssuanceRows']) is int and value['unclassifiedOrphanIssuanceRows'] == 0,
        'unclassified orphan issuance')
    for key in COUNTS:
        need(type(value[key]) is int and 0 <= value[key] <= 2_000_000, 'row bound: ' + key)
    for which in ('checkpoint', 'final'):
        need(value[which+'Rows'] >= value[which+'NativeAccountRows'] + value[which+'TokenAccountRows']
            + value[which+'TokenIssuanceRows'] + 1, 'incomplete prefix row counts')
        need(type(value[which+'Bytes']) is int and 0 <= value[which+'Bytes'] <= 512*1024**2, 'byte bound')
        decimal(value[which+'NativeIssuance'])
    identity.digest_hex(value['legacyMarkerIdentitySha256'])
    rows = value['legacyMarkerRows']
    need(isinstance(rows, list) and len(rows) <= min(value['checkpointTokenIssuanceRows'], value['finalTokenIssuanceRows']), 'marker row bound')
    assets, pools = [], set()
    for row in rows:
        need(isinstance(row, dict) and set(row) == {'asset', 'pool', 'retainedLegacyIssuance'}, 'marker row schema')
        for field in ('asset', 'pool'):
            need(isinstance(row[field], str) and row[field].startswith('0x'), 'identity prefix')
            identity.digest_hex(row[field][2:])
        need(decimal(row['retainedLegacyIssuance']) > 0, 'marker stale liability is not positive')
        assets.append(row['asset']); need(row['pool'] not in pools, 'duplicate marker pool'); pools.add(row['pool'])
    need(assets == sorted(set(assets)), 'duplicate/unordered marker identities')
    return value
