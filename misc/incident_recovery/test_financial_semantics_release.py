import copy
import unittest
import financial_semantics_release as financial


def example_financial():
    return {'schema':'sora-nexus-read-only-cross-record-financial-equations-v1',
        'liquidity':{'providerRows':2,'issuanceRows':1,'poolsWithProviders':1,
            'perPoolProviderSumEqualsIssuance':True,'underlyingAssetReservesVerified':False,
            'checkpointOwnershipConservationVerified':False},
        'kensetsu':{'positionRows':2,'ownerIndexRows':1,'collateralAggregateRows':1,
            'ownerIndexBijectionVerified':True,'storedCollateralAndDebtSumsVerified':True,
            'unaccruedFeeValuationVerified':False,'depositoryBackingVerified':False,
            'protocolBadDebtCoverageVerified':False,'checkpointLiabilityConservationVerified':False},
        'rows':7,'bytes':512,'completeFinancialCoverageVerified':False,
        'completeStorageSemantics':False,'productionReady':False}


class FinancialReportContract(unittest.TestCase):
    def test_bounded_result_is_accepted_without_mutation_or_gate_output(self):
        value=example_financial();before=copy.deepcopy(value)
        self.assertIsNone(financial.validate(value));self.assertEqual(value,before)
    def test_missing_failed_or_promoted_scope_is_rejected(self):
        for group,key,bad in [(None,'schema','archived'),(None,'productionReady',True),
            (None,'completeStorageSemantics',True),('liquidity','perPoolProviderSumEqualsIssuance',False),
            ('liquidity','underlyingAssetReservesVerified',True),('kensetsu','ownerIndexBijectionVerified',False),
            ('kensetsu','storedCollateralAndDebtSumsVerified',1),('kensetsu','checkpointLiabilityConservationVerified',True)]:
            value=example_financial();(value if group is None else value[group])[key]=bad
            with self.subTest(group=group,key=key):self.assertRaises(ValueError,financial.validate,value)
        self.assertRaises(ValueError,financial.validate,None)
    def test_boolean_counts_overflow_and_prefix_count_disagreement_fail(self):
        for group,key,bad in [(None,'rows',True),(None,'rows',6),(None,'bytes',512*1024**2+1),
            ('liquidity','providerRows',-1),('liquidity','poolsWithProviders',2),
            ('kensetsu','positionRows',2_000_001),('kensetsu','ownerIndexRows',False)]:
            value=example_financial();(value if group is None else value[group])[key]=bad
            with self.subTest(group=group,key=key):self.assertRaises(ValueError,financial.validate,value)

if __name__=='__main__':unittest.main()
