"""Synthetic result-contract negatives; never native accounting proof."""
import copy
import unittest
import account_ledger_release as ledger

def example_ledger(post):
    value = {'schema':'sora-nexus-full-account-ledger-equations-v1',
        'checkpointStateRoot':post['checkpointStateRoot'], 'checkpointMetadataHash':'0x'+'ab'*32,
        'finalMetadataHash':post['finalMetadataBlake2'], 'unclassifiedOrphanIssuanceRows':0,
        'legacyMarkerRows':[], 'legacyMarkerIdentitySha256':'cd'*32,
        'checkpointNativeIssuance':'0', 'finalNativeIssuance':'0',
        'checkpointBytes':16, 'finalBytes':16}
    value.update({k:True for k in ledger.TRUE}); value.update({k:False for k in ledger.FALSE})
    value.update({k:0 for k in ledger.COUNTS}); value.update(checkpointRows=1,finalRows=1)
    return value

class LedgerTests(unittest.TestCase):
    def test_complete_equations_keep_historical_and_full_scope_false(self):
        post={'checkpointStateRoot':'0x'+'11'*32,'finalMetadataBlake2':'0x'+'22'*32}
        value=example_ledger(post);self.assertIs(ledger.validate(value,post),value)
        for key in ledger.TRUE+ledger.FALSE:
            bad=copy.deepcopy(value);bad[key]=not bad[key]
            self.assertRaises(ValueError,ledger.validate,bad,post)
    def test_missing_orphan_duplicate_and_forged_amounts_fail(self):
        post={'checkpointStateRoot':'0x'+'11'*32,'finalMetadataBlake2':'0x'+'22'*32}
        value=example_ledger(post)
        for key,invalid in [('unclassifiedOrphanIssuanceRows',1),('unclassifiedOrphanIssuanceRows',False),
            ('checkpointRows',0),('finalBytes',True),('finalNativeIssuance','00'),('finalNativeIssuance',str(2**128)),
            ('finalMetadataHash','0x'+'33'*32),('checkpointMetadataHash','bad')]:
            with self.subTest(key=key):
                bad=copy.deepcopy(value);bad[key]=invalid;self.assertRaises(ValueError,ledger.validate,bad,post)
        value.update(checkpointTokenIssuanceRows=2,finalTokenIssuanceRows=2,checkpointRows=3,finalRows=3)
        marker={'asset':'0x'+'ab'*32,'pool':'0x'+'cd'*32,'retainedLegacyIssuance':'17'}
        value['legacyMarkerRows']=[marker];ledger.validate(value,post)
        value['legacyMarkerRows']=[marker,marker];self.assertRaises(ValueError,ledger.validate,value,post)

if __name__=='__main__':unittest.main()
