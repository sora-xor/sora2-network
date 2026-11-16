"""Synthetic host joins only. No runtime or signature execution is simulated as proof."""
from test_financial_semantics_release import example_financial
from test_account_ledger_release import example_ledger
from test_position_conservation_release import example_conservation
from test_bridge_state_release import example_bridge
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import artifact_identity_release as identity
import storage_semantics_release as storage
from test_maintenance_observer_release import observations
import production_build as build
import release_package as release
import hashi_codec as scale
from test_artifact_identity_release import ArtifactIdentityTests
from test_release_package import policy, h, hx, operators, trusted_witness
from test_production_build import synthetic_public_consensus

class StorageSemanticsTests(unittest.TestCase):
    def setUp(self):
        self.f = ArtifactIdentityTests('runTest'); self.f.setUp(); self.addCleanup(self.f.doCleanups)
        mock=patch.object(build.abandoned,'verify_directory',side_effect=synthetic_public_consensus)
        mock.start();self.addCleanup(mock.stop)
        self.f.put('operatorsScale', operators((2, [9], [10], [11])))
        self.f.chain['recovery'].update(previousGrandpaSetId=7956, firstRecoverySlot=298871045, lastObservedAbandonedSlot=298871044)
        for manifest in (self.f.activation,self.f.hold):
            manifest['previousGrandpaSet']=7956
            for step in manifest['steps']:
                step['slot']+=298870944;step['timestamp']=step['slot']*6000+2
        self.f.put('chainSpec',self.f.chain)
        self.f.put('signedMappingCsv',f'"old account address","new account address"\n"{hx(1)}","{hx(2)}"\n'.encode(),'mapping.csv')
        for role in ['semanticPlan','stakingRoleReview','storageInventory','economic']:
            self.f.put(role,{},role+'.json')
        self.f.put('derivedAccountInventory',b'[]','derived.json')
        self.f.put('council',json.dumps([hx(3)]).encode(),'council.json')
        self.f.put('apply',b'snxrec01\x01\x01synthetic-apply','apply.scale')
        self.f.put('audit',b'snxrec01\x01\x02synthetic-audit','audit.scale')
        self.f.activation['steps']=[self.f.step(298871045,'apply',134,'Applying'),self.f.step(298871046,'audit',134,'Auditing'),self.f.step(298871047,'terminal',138,'Complete'),self.f.step(298871048,None,138,'Complete')]
        compose={'capture':'capture','csv':'mapping.csv','inventory':'storageInventory.json','economic':'economic.json','semantic':'semanticPlan.json','staking-roles':'stakingRoleReview.json'}
        roles=['checkpointCaptureManifest','checkpointCode','operatorsScale','signedMappingCsv','semanticPlan','stakingRoleReview','storageInventory','economic']
        self.inputs={'schema':build.SCHEMA,'authorizationDomain':build.release.policy_identity(policy())['authorizationDomain'],'files':{self.f.records[r]['sourcePath']:{k:self.f.records[r][k] for k in ('sha256','blake2_256','bytes')} for r in roles},'compose':compose,'operatorsScale':'operatorsScale','mixedLayout':None,'consensus':{k:self.f.chain['recovery'][k] for k in ('previousGrandpaSetId','firstRecoverySlot','lastObservedAbandonedSlot')}}
        self.inputs['consensus']={'publicCapture':'public-consensus'}
        for name in ('started.json','headers.json','completion.json'):
            role='public-'+name;self.f.put(role,{'syntheticUnexecutedCapture':name},'public-consensus/'+name)
            self.inputs['files'][self.f.records[role]['sourcePath']]=build.digest(self.f.paths[role])
        self.save_inputs()

    def save_inputs(self):
        commitment=build.pin_bytes(build.serialized(self.inputs))['blake2_256']
        self.policy=policy()[:-64]+bytes.fromhex(commitment)+policy()[-32:]
        self.f.put('policyScale',self.policy)
        for manifest in (self.f.activation,self.f.hold): manifest['policy']=self.f.ref('policyScale')
        self.f.hold['suppliedCouncil']['expectedTerminal']['activePolicyCommitment']='0x'+identity.blake(self.policy)
        self.f.save_manifests()
        self.f.put(storage.ROLE,self.inputs,'compose-inputs.json')

    def bind(self):
        context=self.f.bind()
        return storage.bind(self.f.root,self.f.records,self.f.paths,context,release.local_file,release.digest_file)

    def composed(self):
        return {'schema':'sora-nexus-storage-work-v1','policyScale':'0x'+self.policy.hex(),
            'applyEnvelopes':['0x'+self.f.paths['apply'].read_bytes().hex()],
            'auditEnvelopes':['0x'+self.f.paths['audit'].read_bytes().hex()],
            'terminalEnvelope':'0x'+self.f.paths['terminal'].read_bytes().hex(),
            'storageEngineRehearsed':True,'productionReady':False,
            'trustedMapping':trusted_witness({role:row['sha256'] for role,row in self.f.records.items()})}

    def test_exact_input_graph_and_policy_bind_without_gate_authority(self):
        context=self.bind()
        self.assertFalse(context['completeStorageSemantics'])
        self.assertEqual(len(context['composeGraphRoles']),len(self.inputs['files']))

    def test_no_manifest_means_no_storage_admission(self):
        self.f.paths.pop(storage.ROLE)
        self.assertIsNone(storage.bind(self.f.root,self.f.records,self.f.paths,None,release.local_file,release.digest_file))

    def test_missing_capture_page_and_data_commitment_change_fail(self):
        self.inputs['files'].pop('capture/runtime-code.wasm');self.save_inputs()
        with self.assertRaisesRegex(ValueError,'omitted'):self.bind()

    def test_wrong_mapping_role_and_operator_context_fail(self):
        self.f.put('other',b'[]','other.json');self.inputs['operatorsScale']='other.json'
        self.inputs['files']['other.json']=build.digest(self.f.paths['other']);self.save_inputs()
        with self.assertRaises(ValueError):self.bind()

    def test_fixture_public_keys_and_unsafe_graph_paths_fail(self):
        for mutation in ('fixture','key','path'):
            with self.subTest(mutation=mutation):
                old=copy.deepcopy(self.inputs)
                if mutation=='fixture': self.inputs['schema']='sora-nexus-fixture-v1'
                elif mutation=='path': self.inputs['files']['../escape']={'sha256':'00'*32,'blake2_256':'00'*32,'bytes':0}
                else:
                    self.f.put('signedMappingCsv', ('old account address,new account address\n'+hx(1)+',0x'+next(iter(build.PUBLIC_SIGNING_KEYS))+'\n').encode(),'mapping.csv')
                    self.inputs['files']['mapping.csv']=build.digest(self.f.paths['signedMappingCsv'])
                self.save_inputs()
                with self.assertRaises(ValueError): self.bind()
                self.inputs=old

    def test_exact_canonical_order_including_terminal_is_required(self):
        context=self.bind();composed=self.composed()
        reader=lambda role:self.f.paths[role].read_bytes()
        storage.compare_composition(context,composed,None,self.policy,reader)
        for key in ('applyEnvelopes','auditEnvelopes','terminalEnvelope'):
            bad=copy.deepcopy(composed)
            if isinstance(bad[key],list):bad[key]=bad[key]+bad[key]
            else:bad[key]+='00'
            with self.assertRaises(ValueError):storage.compare_composition(context,bad,None,self.policy,reader)
        with self.assertRaisesRegex(ValueError,'policy differs'):storage.compare_composition(context,composed,None,self.policy+b'0',reader)

    def test_native_composer_commands_use_bounded_large_output_only_for_composition(self):
        context=self.bind();out=self.f.root/'real-compose-wrapper';out.mkdir();calls=[]
        def process(command,cwd,output,guard,**kwargs):
            calls.append((command[1],kwargs));guard()
            if command[1]=='verify-trusted-mapping':return json.dumps(self.composed()['trustedMapping']).encode(),{}
            if command[1]=='derive-checkpoint-consensus':
                raw=self.f.paths['operatorsScale'].read_bytes(); witness=self.composed()['trustedMapping']
                return json.dumps({'schema':'sora-nexus-checkpoint-consensus-v2',
                    'checkpointHash':scale.CHECKPOINT_HASH,'checkpointNumber':scale.CHECKPOINT,
                    'checkpointStateRoot':release.CHECKPOINT_ROOT,'captureManifestSha256':self.f.records['checkpointCaptureManifest']['sha256'],
                    'checkpointMetadata':witness['checkpointMetadata'],'checkpointCouncil':witness['checkpointCouncil'],
                    'signingKeysPreserved':True,'manualOperatorApprovalRequired':False,'productionReady':False,'checkpointSlot':298870000,
                    'previousGrandpaSetId':7956,'grandpaAuthorities':[[hx(9),1]],'previousGrandpaAuthorities':[[hx(9),1]],
                    'approvedGrandpaKeys':[hx(9)],'operatorsScale':'0x'+raw.hex(),'operatorsBlake2':'0x'+build.pin_bytes(raw)['blake2_256']}).encode(),{}
            if command[1]=='compose-storage-work':return json.dumps(self.composed()).encode(),{}
            if command[1]=='emit-final-policy':
                return ('pub const POLICY_SCALE: &[u8] = &['+', '.join(str(b) for b in self.policy)+'];').encode(),{}
            self.fail('unexpected native command')
        with patch.object(build,'process',side_effect=process):
            policy_bytes,_=build.compose_inputs(self.inputs,self.f.root,out,self.f.root/'unused-native',lambda:None,context['composeDataCommitment'])
        self.assertEqual(policy_bytes,self.policy)
        self.assertEqual([name for name,_ in calls],['derive-checkpoint-consensus','verify-trusted-mapping','compose-storage-work','emit-final-policy'])
        self.assertEqual(calls[2][1]['maximum'],512*1024**2)
        self.assertGreaterEqual(calls[2][1]['timeout'],6*3600)
        self.assertNotIn('maximum',calls[0][1]);self.assertNotIn('maximum',calls[3][1])
        with self.assertRaises(ValueError):build.compose_inputs({},self.f.root,out,Path('none'),lambda:None,'00'*32,composition_limit=2**64)

    def post(self,result,context):
        last=result['blocks'][-1]
        post = {'schema':'sora-nexus-fresh-post-state-semantics-v1','checkpointStateRoot':identity.history.ROOT,'captureManifestSha256':context['captureManifestSha256'],'checkpointMetadataWitness':{'checkpointHash':scale.CHECKPOINT_HASH,'stateRoot':identity.history.ROOT,'executingCodeBlake2':'0x'+context['checkpointCodeBlake2'],'wasmExecutionVerified':True,'nativeExecution':False,'productionReady':False},'finalStateRoot':last['stateRoot'],'finalBlockHash':last['hash'],'executingCodeBlake2':context['wasm']['138']['blake2'],'policyCommitment':context['policyCommitment'],'terminalAuditCommitment':'07'*32,'finalMetadataSha256':identity.sha(b'synthetic final metadata'),'finalMetadataBlake2':hx(12),'finalMetadataExecutedAsWasm':True,'finalMetadataNativeExecution':False,'committedTerminalFinancialInvariantsRechecked':True,'retirementAbsenceWitnessesRechecked':1,'issuanceEquationsRechecked':1,'collateralEquationsRechecked':0,'zeroClaimsRechecked':0,'fullFinancialLiabilityCoverageVerified':False,'retiredKeyDispatchRejectionVerified':False,'readOnlyStateRootMatches':True,'completeStorageSemantics':False,'productionReady':False,'maintenanceStateObservations':observations(result,context),'crossRecordFinancialEquations':example_financial(),'accountLedgerEquations':example_ledger({'checkpointStateRoot':identity.history.ROOT,'finalMetadataBlake2':hx(12)}),'bridgeStateEquations':example_bridge(result,context),'scan':{'entries':1,'bytes':32,'childTries':0,'metadataDecodedEntries':0,'retiredTypedReferences':0,'issues':{'unknown metadata':1},'examples':[],'completeTrieEnumerated':True,'allEntriesTyped':False,'typedScanSha256':'cd'*32,'historicalReferencesDispositionVerified':False,'derivedAccountDiscoveryComplete':False,'completeMutableOwnershipVerified':False}}
        post['positionConservation']=example_conservation(post,last)
        return post

    def test_fresh_final_state_root_code_and_false_scope_are_required(self):
        context=self.bind();result,context,out,pins=self.f.witness(context=context)
        result['postStateSemantics']=self.post(result,context)
        (out/'post-state-metadata.scale').write_bytes(b'synthetic final metadata');pins[str(out/'post-state-metadata.scale')]=build.digest(out/'post-state-metadata.scale')
        storage.validate(result,context,'activation',out,pins)
        for key,value in [('finalStateRoot',hx(1)),('executingCodeBlake2','ff'*32),('completeStorageSemantics',True),('readOnlyStateRootMatches',False),('issuanceEquationsRechecked',True),('crossRecordFinancialEquations',None),('positionConservation',None)]:
            bad=copy.deepcopy(result);bad['postStateSemantics'][key]=value
            with self.assertRaises(ValueError):storage.validate(bad,context,'activation',out,pins)
        result.pop('postStateSemantics')
        with self.assertRaisesRegex(ValueError,'same-process'):storage.validate(result,context,'activation',out,pins)

    def run_mock(self,mutation=None,compose_failure=False):
        context=self.bind(); report={'storageSemanticsReplay':context,'artifacts':list(self.f.records.values()),'gates':{storage.GATE:{'status':'unverified'},identity.GATE:{'status':'unverified'},'accountOwnership':{'status':'verified','evidence':'unchanged'}},'productionReady':False}
        initial=copy.deepcopy(report['gates']);paths=dict(self.f.paths);count=len(report['artifacts']);order=[]
        binary=self.f.root/'binary';binary.write_bytes(b'not executed');binary.chmod(0o555)
        def compose(value,inputs,output,binary_arg,guard,commitment,remember,retain):
            order.append('compose');guard()
            if compose_failure:raise ValueError('native composition rejected')
            remember(output/'compose/stdout.log',json.dumps(self.composed()).encode())
            remember(output/'policy.scale',self.policy)
            remember(output/'checkpoint-operators.scale',self.f.paths['operatorsScale'].read_bytes())
            guard();return self.policy,b'generated'
        def process(command,cwd,output,guard,**kwargs):
            order.append('replay');self.assertEqual(command[1],'rehearse-storage-semantics');self.assertGreaterEqual(kwargs['timeout'],86400)
            guard();output.mkdir(parents=True)
            original=self.f.paths
            self.f.paths={role:Path(cwd)/'inputs'/self.f.records[role]['sourcePath'] for role in context['inputRoles']}
            try:result,bound,_,_=self.f.witness(context=context,output=output/'replay')
            finally:self.f.paths=original
            result['postStateSemantics']=self.post(result,bound)
            (output/'replay/post-state-metadata.scale').write_bytes(b'synthetic final metadata')
            raw=json.dumps(result).encode();receipt={'outputs':{}}
            for name,value in [('launch.json',{'mock':True}),('started.json',{'pid':1}),('stdout.log',raw),('stderr.log',b'')]:
                pin=build.write(output/name,value);kwargs['retain'](output/name,pin)
                if name.endswith('.log'):receipt['outputs'][name]=pin
            pin=build.write(output/'execution.json',receipt);kwargs['retain'](output/'execution.json',pin)
            return raw,receipt
        actual=storage.validate
        def validate(result,bound,scenario,out,pins):
            actual(result,bound,scenario,out,pins)
            if mutation:mutation(out,bound)
        with patch.object(build,'compose_inputs',side_effect=compose),patch.object(build,'process',side_effect=process),patch.object(storage,'validate',side_effect=validate):
            try:storage.run(report,paths,binary,identity.sha(binary.read_bytes()),self.f.root/'runs',release.local_file,release.digest_file)
            except BaseException:
                self.assertEqual(report['gates'],initial);self.assertEqual(len(report['artifacts']),count)
                self.assertFalse(any(r.startswith('storageSemanticsOutput-') for r in paths));raise
        self.assertEqual(order,['compose','replay']);self.assertEqual(report['gates'][storage.GATE]['status'],'unverified')
        self.assertTrue(report['gates'][storage.GATE]['freshSupportingEvidenceVerified'])
        self.assertEqual({k:v for k,v in report['gates'].items() if k not in (storage.GATE,storage.observer.GATE,storage.bridge.GATE)},{k:v for k,v in initial.items() if k!=storage.GATE})
        self.assertEqual(report['gates'][storage.observer.GATE]['status'],'unverified')
        self.assertEqual(report['gates'][storage.bridge.GATE]['status'],'unverified')
        self.assertFalse(report['gates'][storage.observer.GATE]['evidence']['requiredBoundarySequenceExercised'])
        self.assertFalse(report['productionReady'])
        return report

    def test_fresh_composition_precedes_replay_but_no_gate_is_promoted(self): self.run_mock()
    def test_failed_composition_cannot_be_replaced_by_archive_or_start_replay(self):
        with self.assertRaisesRegex(ValueError,'native composition rejected'): self.run_mock(compose_failure=True)
    def test_late_snapshot_mutation_cannot_register_evidence(self):
        with self.assertRaises(ValueError):self.run_mock(lambda out,_:(out/'snapshots/000001.json').write_bytes(b'changed'))
    def test_late_composition_mutation_cannot_register_evidence(self):
        with self.assertRaises(ValueError):self.run_mock(lambda out,_:(out.parent.parent/'composition/compose/stdout.log').write_bytes(b'changed'))
    def test_late_metadata_mutation_cannot_register_evidence(self):
        with self.assertRaises(ValueError):self.run_mock(lambda out,_:(out/'post-state-metadata.scale').write_bytes(b'changed'))
    def test_late_input_mutation_cannot_register_evidence(self):
        def mutate(_,bound):
            path=Path(bound['inputPaths']['signedMappingCsv']);path.chmod(0o644);path.write_bytes(b'changed')
        with self.assertRaises(ValueError):self.run_mock(mutate)

if __name__=='__main__':unittest.main()
