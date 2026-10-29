"""Synthetic validator regressions only; no snapshot/runtime result is fabricated as evidence."""
import copy,unittest
import maintenance_observer_release as observer

def observations(result,context):
    rows=[];epoch=100;session=7;boundary=None;followups=0;changes=0
    for block,detail in zip(result['blocks'],context['scenarios']['activation']['steps']):
        maintenance=detail['executingSpec']!=138;changed=False
        if not maintenance and boundary is None and result['livenessGraceObserved']:
            boundary=block['number'];epoch+=1;session+=1;changed=True
        elif boundary is not None:followups+=1
        rows.append({'block':block['number'],'blockHash':block['hash'],'stateRoot':block['stateRoot'],
            'snapshotManifestSha256':block['diskSnapshotManifestSha256'],'executingCodeBlake2':block['executingCodeBlake2'],
            'maintenance':maintenance,'epoch':epoch,'session':session,'slot':detail['step']['slot'],
            'epochChanged':changed,'currentEraScale':'01000000','activeEraScale':'0100000000',
            'livenessResumedAt':boundary,'tracking':None if boundary is None else [session,[]],
            'ordinaryStakingEquations':None,'readOnlyStateRootMatches':True})
    return {'schema':observer.SCHEMA,'fixtureOnly':False,'epochDurationFromExecutedWasm':600,'slotDurationFromExecutedWasm':6000,
        'maintenanceEpochTransitionsVerified':changes,'maintenanceEpochBoundaryExercised':False,
        'frozenEconomicSessionAndEraVerified':True,'announcedBabeTransitionsVerified':True,'boundedMaintenanceEpochAttributionVerified':True,
        'deferredBoundaryBlock':boundary,'deferredGraceStateEffectsVerified':boundary is not None,'postBoundaryTrackingBlocks':followups,'trackingContinuationExercised':followups>0,
        'signedStakingLedgerEquations':None,'rows':rows,'fullElectionCorrectnessVerified':False,'fullExposureAndRewardCorrectnessVerified':False,'heartbeatSignaturesVerified':False,
        'maintenanceEpochLivenessAndStakingVerified':False,'consensusSealsVerified':False,'operatorApprovalVerified':False,'productionReady':False}

def fixture():
    steps=[{'executingSpec':n,'step':{'slot':100+i}} for i,n in enumerate([134,134,138,138,138])]
    blocks=[{'number':28191818+i,'hash':'0x'+f'{i:064x}','stateRoot':'0x'+f'{i+10:064x}',
             'diskSnapshotManifestSha256':f'{i+20:064x}','executingCodeBlake2':f'{n:064x}'} for i,n in enumerate([134,134,138,138,138])]
    context={'scenarios':{'activation':{'steps':steps}}}
    result={'blocks':blocks,'publicTestCouncilFixture':False,'publicTestHashiFixture':False,'publicSignedTransactions':None,'livenessGraceObserved':True,
            'manifestSha256':'ab'*32,'replayTranscriptSha256':'cd'*32}
    result['postStateSemantics']={'maintenanceStateObservations':observations(result,context)}
    return result,context

class ObserverTests(unittest.TestCase):
    def test_partial_real_scope_never_promotes_unexercised_or_signed_claims(self):
        result,context=fixture();report=observer.validate(result,context,'activation')
        self.assertFalse(report['requiredBoundarySequenceExercised']);self.assertFalse(report['signedStakingExecuted'])
        self.assertEqual(report['postBoundaryTrackingBlocks'],2)
        gate=observer.supporting_gate(report);self.assertEqual(gate['status'],'unverified');self.assertFalse(gate['productionReady'])
    def test_actual_row_boundary_count_is_required(self):
        result,context=fixture();value=result['postStateSemantics']['maintenanceStateObservations']
        for index,row in enumerate(value['rows']):
            if index>0:row['epoch']+=1
        value['rows'][1]['epochChanged']=True;value['maintenanceEpochTransitionsVerified']=1;value['maintenanceEpochBoundaryExercised']=True
        self.assertTrue(observer.validate(result,context,'activation')['requiredBoundarySequenceExercised'])
        value['maintenanceEpochTransitionsVerified']=2
        self.assertRaises(ValueError,observer.validate,result,context,'activation')
    def test_public_or_claimed_staking_projections_cannot_enter_real_scope(self):
        result,context=fixture()
        for target,key,bad in [('result','publicTestCouncilFixture',True),('result','publicTestHashiFixture',True),('result','publicSignedTransactions',{}),
            ('value','fixtureOnly',True),('value','schema','sora-nexus-public-fixture-maintenance-state-observations-v1'),('value','signedStakingLedgerEquations',{}),
            ('value','fullElectionCorrectnessVerified',True),('value','heartbeatSignaturesVerified',True),('value','productionReady',True)]:
            with self.subTest(key=key):
                changed=copy.deepcopy(result);destination=changed if target=='result' else changed['postStateSemantics']['maintenanceStateObservations'];destination[key]=bad
                self.assertRaises(ValueError,observer.validate,changed,context,'activation')
    def test_missing_or_detached_native_rows_fail(self):
        result,context=fixture()
        for key,bad in [('blockHash','0x'+'ef'*32),('stateRoot','0x'+'ef'*32),('snapshotManifestSha256','ef'*32),('executingCodeBlake2','ef'*32),('slot',9000),('ordinaryStakingEquations',{}),('epochChanged',1),('readOnlyStateRootMatches',False)]:
            with self.subTest(key=key):
                changed=copy.deepcopy(result);changed['postStateSemantics']['maintenanceStateObservations']['rows'][1][key]=bad
                self.assertRaises(ValueError,observer.validate,changed,context,'activation')
        result['postStateSemantics'].clear();self.assertRaises(ValueError,observer.validate,result,context,'activation')
    def test_frozen_era_grace_and_tracking_cannot_be_faked_by_flags(self):
        result,context=fixture()
        mutations=[(1,'currentEraScale','02000000'),(1,'session',99),(2,'livenessResumedAt',None),(3,'tracking',[999,[]]),(3,'epoch',99)]
        for index,key,bad in mutations:
            with self.subTest(key=key):
                changed=copy.deepcopy(result);changed['postStateSemantics']['maintenanceStateObservations']['rows'][index][key]=bad
                self.assertRaises(ValueError,observer.validate,changed,context,'activation')
        for key,bad in [('maintenanceEpochTransitionsVerified',True),('postBoundaryTrackingBlocks',True),('deferredBoundaryBlock',28191818),('trackingContinuationExercised',False)]:
            changed=copy.deepcopy(result);changed['postStateSemantics']['maintenanceStateObservations'][key]=bad
            self.assertRaises(ValueError,observer.validate,changed,context,'activation')
if __name__=='__main__':unittest.main()
