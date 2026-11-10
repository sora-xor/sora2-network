"""Join fresh nonfixture epoch/grace observations; never certify full staking."""
import artifact_identity_release as identity

GATE='maintenanceEpochLivenessAndStaking'
SCHEMA='sora-nexus-nonfixture-maintenance-state-observations-v1'
REMAINING=[
    'Post-activation election, exposure and reward distribution need executable exact-artifact scenarios; current state equations and public bond/unbond examples do not prove these paths.',
    'Actual heartbeat signatures and subsequent session liveness/offence behavior require a dedicated admitted scenario; the deferred grace state effect alone does not prove them.',
    'A maintenance epoch boundary, deferred boundary and later tracking blocks must actually occur; unexercised boundaries remain explicit even after read-only observation succeeds.',
]

def require(ok,why):identity.require(ok,'maintenance observer: '+why)
def uint(value,limit=2**64-1):
    require(type(value) is int and 0<=value<=limit,'bounded integer required');return value

def validate(result,context,scenario):
    require(scenario=='activation' and result.get('publicTestCouncilFixture') is False
        and result.get('publicTestHashiFixture') is False and result.get('publicSignedTransactions') is None,
        'only real normal activation observations are admitted')
    value=result['postStateSemantics'].get('maintenanceStateObservations')
    require(isinstance(value,dict) and value.get('schema')==SCHEMA and value.get('fixtureOnly') is False,
        'fresh nonfixture native observer is required')
    for key in ('frozenEconomicSessionAndEraVerified','announcedBabeTransitionsVerified','boundedMaintenanceEpochAttributionVerified'):
        require(value.get(key) is True,'native equation check missing: '+key)
    for key in ('fullElectionCorrectnessVerified','fullExposureAndRewardCorrectnessVerified','heartbeatSignaturesVerified','maintenanceEpochLivenessAndStakingVerified','consensusSealsVerified','operatorApprovalVerified','productionReady'):
        require(value.get(key) is False,'unsupported scope: '+key)
    require(value.get('signedStakingLedgerEquations') is None,'public signed staking claims cannot enter real observations')
    duration=uint(value.get('epochDurationFromExecutedWasm'),1_000_000);slot_duration=uint(value.get('slotDurationFromExecutedWasm'))
    require(duration>0 and slot_duration>0,'executed BABE API durations absent')
    rows=value.get('rows');blocks=result['blocks'];steps=context['scenarios'][scenario]['steps']
    require(isinstance(rows,list) and len(rows)==len(blocks)==len(steps) and len(rows)>0,'complete fresh block observation sequence required')
    changed=0;boundary=None;followups=0;prior=None;maintenance_baseline=None
    for row,block,detail in zip(rows,blocks,steps):
        require(isinstance(row,dict),'invalid native observer row')
        for key,source in [('block','number'),('blockHash','hash'),('stateRoot','stateRoot'),('snapshotManifestSha256','diskSnapshotManifestSha256'),('executingCodeBlake2','executingCodeBlake2')]:
            require(row.get(key)==block[source],'row detached from fresh restored block: '+key)
        maintenance=detail['executingSpec']!=138
        require(row.get('maintenance') is maintenance and row.get('slot')==detail['step']['slot']
            and row.get('readOnlyStateRootMatches') is True and row.get('ordinaryStakingEquations') is None,
            'scope/slot/read-only row differs')
        epoch=uint(row.get('epoch'));session=uint(row.get('session'),2**32-1)
        require(type(row.get('epochChanged')) is bool,'epoch transition must be boolean')
        if prior:
            require(epoch>=prior['epoch'] and row['epochChanged'] is (epoch!=prior['epoch']), 'epoch transition differs from preceding row')
            require(session==prior['session']+(0 if maintenance or not row['epochChanged'] else 1),'session transition differs')
        if maintenance:
            baseline=(session,row.get('currentEraScale'),row.get('activeEraScale'))
            if maintenance_baseline is None:maintenance_baseline=baseline
            require(baseline==maintenance_baseline and row.get('livenessResumedAt') is None,'maintenance economic state changed')
            changed+=int(row['epochChanged'])
        else:
            if boundary is None and row['epochChanged']:
                boundary=block['number']
            require(row.get('livenessResumedAt')==boundary,'one-shot deferred grace marker differs')
            if boundary is not None:
                tracking=row.get('tracking');require(isinstance(tracking,list) and len(tracking)==2 and tracking[0]==session,'resumed tracking does not match current session')
                if not row['epochChanged']:followups+=1
        prior=row
    require(value.get('maintenanceEpochTransitionsVerified')==changed and type(value.get('maintenanceEpochTransitionsVerified')) is int
        and value.get('maintenanceEpochBoundaryExercised') is (changed>0),'maintenance boundary coverage differs')
    require(result.get('livenessGraceObserved') is (boundary is not None), 'top-level liveness observation differs')
    require(value.get('deferredBoundaryBlock')==boundary and (boundary is None or type(value['deferredBoundaryBlock']) is int)
        and value.get('deferredGraceStateEffectsVerified') is (boundary is not None),'deferred boundary coverage differs')
    require(type(value.get('postBoundaryTrackingBlocks')) is int and value['postBoundaryTrackingBlocks']==followups
        and value.get('trackingContinuationExercised') is (followups>0),'following tracking coverage differs')
    return {'schema':'sora-nexus-fresh-nonfixture-maintenance-observer-evidence-v1',
        'manifestSha256':result['manifestSha256'],'replayTranscriptSha256':result['replayTranscriptSha256'],
        'finalStateRoot':blocks[-1]['stateRoot'],'finalBlockHash':blocks[-1]['hash'],
        'observedBlocks':len(rows),'maintenanceEpochTransitions':changed,'deferredBoundaryBlock':boundary,
        'postBoundaryTrackingBlocks':followups,'epochConfigurationAndFrozenSessionEquationsVerified':True,
        'requiredBoundarySequenceExercised':changed>0 and boundary is not None and followups>0,
        'signedStakingExecuted':False,'fullCustomStakingVerified':False,'heartbeatSignaturesVerified':False,
        'fixtureOnly':False,'productionReady':False}


def supporting_gate(evidence):
    # Invoked before run_owned's final guarded registration, never on imported JSON.
    require(evidence.get('schema')=='sora-nexus-fresh-nonfixture-maintenance-observer-evidence-v1'
        and evidence.get('fixtureOnly') is False and evidence.get('productionReady') is False,'native observer evidence scope differs')
    return {'status':'unverified','reason':'Fresh nonfixture epoch/configuration/frozen-session and deferred-grace observations passed; full custom staking and heartbeat scenarios remain incomplete',
        'freshSupportingEvidenceVerified':True,'coverageScope':'readOnlyEpochAndDeferredGrace',
        'evidence':evidence,'remaining':REMAINING,'productionReady':False}
