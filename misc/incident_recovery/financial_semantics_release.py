"""Validate the bounded cross-record result within fresh native storage replay.

This function alone is not execution evidence. Its caller must first verify the
owned exact replay, output graph, executing runtime and same-process state root.
It never produces a certificate or clears a release gate.
"""

def require(value, message):
    if not value:
        raise ValueError('storage financial equations: ' + message)


def validate(value):
    require(isinstance(value, dict) and set(value) == {'schema', 'liquidity', 'kensetsu',
        'rows', 'bytes', 'completeFinancialCoverageVerified', 'completeStorageSemantics', 'productionReady'},
        'native equation result absent or schema differs')
    require(value['schema'] == 'sora-nexus-read-only-cross-record-financial-equations-v1', 'native schema differs')
    for name in ('completeFinancialCoverageVerified', 'completeStorageSemantics', 'productionReady'):
        require(value[name] is False, 'unsupported broad coverage: ' + name)
    lp, debt = value['liquidity'], value['kensetsu']
    require(isinstance(lp, dict) and set(lp) == {'providerRows', 'issuanceRows', 'poolsWithProviders',
        'perPoolProviderSumEqualsIssuance', 'underlyingAssetReservesVerified', 'checkpointOwnershipConservationVerified'},
        'LP result shape differs')
    require(isinstance(debt, dict) and set(debt) == {'positionRows', 'ownerIndexRows', 'collateralAggregateRows',
        'ownerIndexBijectionVerified', 'storedCollateralAndDebtSumsVerified', 'unaccruedFeeValuationVerified',
        'depositoryBackingVerified', 'protocolBadDebtCoverageVerified', 'checkpointLiabilityConservationVerified'},
        'CDP result shape differs')
    for record, names in ((lp, ('perPoolProviderSumEqualsIssuance',)),
                          (debt, ('ownerIndexBijectionVerified', 'storedCollateralAndDebtSumsVerified'))):
        for name in names:
            require(record[name] is True, 'native equation did not pass: ' + name)
    for record, names in ((lp, ('underlyingAssetReservesVerified', 'checkpointOwnershipConservationVerified')),
                          (debt, ('unaccruedFeeValuationVerified', 'depositoryBackingVerified',
                                  'protocolBadDebtCoverageVerified', 'checkpointLiabilityConservationVerified'))):
        for name in names:
            require(record[name] is False, 'unproved financial scope: ' + name)
    for record, names in ((lp, ('providerRows', 'issuanceRows', 'poolsWithProviders')),
                          (debt, ('positionRows', 'ownerIndexRows', 'collateralAggregateRows')),
                          (value, ('rows',))):
        for name in names:
            require(type(record[name]) is int and 0 <= record[name] <= 2_000_000, 'native count: ' + name)
    require(type(value['bytes']) is int and 0 <= value['bytes'] <= 512*1024**2, 'native byte bound')
    require(value['rows'] == lp['providerRows'] + lp['issuanceRows'] + debt['positionRows']
        + debt['ownerIndexRows'] + debt['collateralAggregateRows'], 'native complete-prefix row counts differ')
    require(lp['poolsWithProviders'] <= min(lp['providerRows'], lp['issuanceRows']), 'LP pool/row counts differ')
