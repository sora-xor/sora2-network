"""Synthetic native-result contract tests, never certificates or runtime execution."""
import copy
import unittest
import position_conservation_release as conservation


def example_conservation(post, witness, fixture=False, terminal=None):
    value={'schema':'sora-nexus-owner-position-conservation-v1','fixtureOnly':fixture,
        'checkpointStateRoot':post['checkpointStateRoot'],'checkpointMetadataHash':'0x'+'a1'*32,
        'inspectionStateRoot':post['finalStateRoot'],'inspectionBlock':witness['number'],
        'inspectionBlockHash':post['finalBlockHash'],'inspectionSnapshotManifestSha256':witness['diskSnapshotManifestSha256'],
        'finalMetadataHash':post['finalMetadataBlake2'],'certificateTerminalStateRoot':terminal or post['finalStateRoot'],
        'policyCommitment':post['policyCommitment'],'replacementCount':1,
        'checkpointRows':[3,2,1],'inspectionRows':[3,2,1],'checkpointBytes':600,'inspectionBytes':600,
        'checkpointPositionSha256':'b1'*32,'expectedPositionSha256':'b2'*32,'inspectionPositionSha256':'b2'*32,
        'incomingOperations':1,'incomingDeltaRows':2,'incomingJournalSha256':'c1'*32}
    value.update({k:True for k in conservation.TRUE});value.update({k:False for k in conservation.FALSE})
    return value


class PositionContractTests(unittest.TestCase):
    def setUp(self):
        self.post={'checkpointStateRoot':'0x'+'11'*32,'finalStateRoot':'0x'+'22'*32,
            'finalBlockHash':'0x'+'33'*32,'finalMetadataBlake2':'0x'+'44'*32,'policyCommitment':'55'*32}
        self.witness={'number':28191899,'stateRoot':self.post['finalStateRoot'],'hash':self.post['finalBlockHash'],
            'diskSnapshotManifestSha256':'66'*32}
        self.value=example_conservation(self.post,self.witness)
    def check(self,value=None):
        return conservation.validate(value or self.value,self.post,self.witness,self.post['finalStateRoot'])
    def test_exact_family_equations_preserve_false_broad_scope(self):
        self.assertIs(self.check(),self.value)
        for key in conservation.TRUE+conservation.FALSE:
            with self.subTest(key=key):
                bad=copy.deepcopy(self.value);bad[key]=not bad[key]
                self.assertRaises(ValueError,self.check,bad)
    def test_missing_contract_and_swapped_owner_digest_fail(self):
        for key in self.value:
            bad=copy.deepcopy(self.value);del bad[key]
            self.assertRaises(ValueError,self.check,bad)
        bad=copy.deepcopy(self.value);bad['inspectionPositionSha256']='ff'*32
        self.assertRaisesRegex(ValueError,'positions differ',self.check,bad)
    def test_checkpoint_metadata_inspection_and_native_terminal_are_distinct_joins(self):
        for key in ('checkpointStateRoot','inspectionStateRoot','inspectionBlockHash','finalMetadataHash',
                    'certificateTerminalStateRoot','inspectionSnapshotManifestSha256','policyCommitment'):
            bad=copy.deepcopy(self.value);bad[key]=('0x' if key not in ('policyCommitment','inspectionSnapshotManifestSha256') else '')+'ff'*32
            self.assertRaises(ValueError,self.check,bad)
        bad=copy.deepcopy(self.value);bad['checkpointMetadataHash']='bad';self.assertRaises(ValueError,self.check,bad)
        bad=copy.deepcopy(self.value);bad['inspectionBlock']=True;self.assertRaises(ValueError,self.check,bad)
    def test_public_early_snapshot_requires_explicit_fixture_and_actual_terminal(self):
        terminal='0x'+'77'*32;value=example_conservation(self.post,self.witness,True,terminal)
        conservation.validate(value,self.post,self.witness,terminal,True)
        self.assertRaises(ValueError,conservation.validate,value,self.post,self.witness,terminal)
        self.assertRaises(ValueError,conservation.validate,value,self.post,self.witness,self.post['finalStateRoot'],True)
    def test_bounds_counts_and_json_boolean_counts_cannot_bypass_native_scope(self):
        for key,value in [('incomingOperations',True),('incomingOperations',16385),('incomingDeltaRows',65537),
                          ('replacementCount',4097),('checkpointRows',[1,1]),('inspectionRows',[2_000_000,1,0]),
                          ('checkpointRows',[True,0,0]),('inspectionBytes',512*1024**2+1)]:
            bad=copy.deepcopy(self.value);bad[key]=value;self.assertRaises(ValueError,self.check,bad)
        bad=copy.deepcopy(self.value);bad['incomingOperations']=0
        self.assertRaisesRegex(ValueError,'without an opaque',self.check,bad)


if __name__=='__main__': unittest.main()
