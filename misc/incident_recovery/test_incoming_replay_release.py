"""Synthetic package binding; no native process or runtime is executed."""
import copy,hashlib,json,unittest
from unittest import mock
from pathlib import Path
import incoming_replay_release as helper
import incoming_release as incoming
import release_package as release
import test_release_package as fixtures

class ReplayPackage(unittest.TestCase):
    setUp=fixtures.ReleaseTests.setUp
    artifact=fixtures.ReleaseTests.artifact
    digest=fixtures.ReleaseTests.digest
    write_candidate=fixtures.ReleaseTests.write_candidate
    full_fixture=fixtures.ReleaseTests.full_fixture

    def prepare(self):
        self.full_fixture()
        original=(self.root/self.inventory['policyScale']['path']).read_bytes()
        active_bytes=original[:167]+b'\x44'*32+original[199:]
        self.artifact('incomingActivePolicyScale',active_bytes,'active.scale')
        work=json.loads((self.root/'work/index.json').read_text());work['policySha256']=self.inventory['incomingActivePolicyScale']['sha256'];self.artifact('recoveryWorkIndex',work,'work/index.json')
        def ref(role):return {'file':self.inventory[role]['path'],'sha256':self.inventory[role]['sha256']}
        self.artifact('repairWasm',fixtures.wasm(136,132),'repair.wasm')
        self.artifact('repair136PolicyScale',active_bytes,'repair-policy.scale')
        binding=json.loads((self.root/'holdBinding').read_text());binding.update(originalPolicyScale='0x'+original.hex(),originalPolicyCommitment=binding['policyCommitment'])
        binding.update(policyScale='0x'+active_bytes.hex(),policyCommitment='0x'+release.policy_identity(active_bytes)['commitment'])
        self.artifact('repair136Binding',binding)
        provenance=json.loads((self.root/'holdProvenance').read_text());provenance.update(specVersion=136,releaseReady=False)
        provenance['bindingEvidence'].update(binding=self.digest('repair136Binding'),policy=self.digest('repair136PolicyScale'))
        self.artifact('repair136Provenance',provenance)
        for name in ['cargo.stdout.log','cargo.stderr.log']:(self.root/name).write_bytes(b'nonexecuted synthetic log')
        build={'status':'passed','exitCode':0,'specVersion':136,'transactionVersion':132,'sourceProvenance':release.digest_file(self.root/'repair136Provenance'),'productionReady':False,
            'artifacts':{'runtime.wasm':release.digest_file(self.root/'repair.wasm')},'logs':{name:release.digest_file(self.root/name) for name in ['cargo.stdout.log','cargo.stderr.log']}}
        self.artifact('repair136BuildReceipt',build)
        binding={'schema':'sora-nexus-native-operation-engine-binding-v1','fixtureOnly':False,'productionReady':False}
        self.artifact('nativeBinding',binding,'native/binding.json');self.artifact('nativeExecutable',b'fake-nonexecuted-native','native/engine');(self.root/'native/engine').chmod(0o555)
        native={'executable':ref('nativeExecutable'),'engineBinding':ref('nativeBinding')}
        active=release.policy_identity(active_bytes)
        manifest={'schema':'sora-nexus-supplied-council-maintenance-replay-v1','checkpointHash':release.scale.CHECKPOINT_HASH,
            'checkpointCapture':str(Path(self.inventory['checkpointCaptureManifest']['path']).parent),'policy':ref('policyScale'),'bootstrap':ref('bootstrapWasm'),'hold':ref('holdWasm'),
            'finalRuntime':ref('finalWasm'),'repairs':[ref('repairWasm')],'previousGrandpaSet':0,'freshGrandpa':[['0x'+'21'*32,1]],'nativeOperationRecorder':native,
            'steps':[{'slot':i,'timestamp':i,'authorityIndex':0,'recovery':None,'expectedSpec':138,'expectedPhase':'Complete'} for i in [1,2,3]],
            'suppliedCouncil':{'escapes':[],'expectedTerminal':{'specVersion':138,'phase':'Complete','activePolicyCommitment':'0x'+active['commitment'],'ordinaryBlocks':1}}}
        self.artifact('recoveryParentReplay',manifest,'replay.json');self.write_candidate();return manifest
    def test_nested_bundle_and_original_policy_remain_bound(self):
        self.prepare();report,paths=release.verify_candidate(self.candidate)
        self.assertNotEqual(report['policy']['commitment'],report['incomingRecoveryReplay']['activePolicy']['commitment'])
        self.assertEqual(report['policy']['mapping'],report['incomingRecoveryReplay']['activePolicy']['mapping'])
        self.assertEqual(report['policy']['commitment'],release.policy_identity((self.root/'policyScale').read_bytes())['commitment'])
        args=incoming.arguments(paths);self.assertEqual(args[args.index('--policy')+1],str(paths['incomingActivePolicyScale']));self.assertIn('--recovery-replay',args);self.assertNotIn('--recovery-output',args)
    def test_missing_active_policy_pair_rejected(self):
        self.prepare();del self.inventory['incomingActivePolicyScale']
        with self.assertRaisesRegex(ValueError,'paired'):release.verify_candidate(self.write_candidate())
    def test_public_fixture_schema_rejected(self):
        m=self.prepare();m['schema']='sora-nexus-public-fixture-prior-policy-audit-maintenance-v1';self.artifact('recoveryParentReplay',m,'replay.json')
        with self.assertRaisesRegex(ValueError,'supplied-council'):release.verify_candidate(self.write_candidate())
    def test_public_native_binding_rejected(self):
        m=self.prepare();self.artifact('nativeBinding',{'schema':'sora-nexus-native-operation-engine-binding-v1','fixtureOnly':True,'productionReady':False},'native/binding.json');m['nativeOperationRecorder']['engineBinding']['sha256']=self.inventory['nativeBinding']['sha256'];self.artifact('recoveryParentReplay',m,'replay.json')
        with self.assertRaisesRegex(ValueError,'public|Public'):release.verify_candidate(self.write_candidate())
    def test_path_escape_rejected(self):
        m=self.prepare();m['repairs'][0]['file']='../outside.wasm';self.artifact('recoveryParentReplay',m,'replay.json')
        with self.assertRaisesRegex(ValueError,'must stay'):release.verify_candidate(self.write_candidate())
    def test_wrong_original_policy_rejected(self):
        m=self.prepare();m['policy']['sha256']='00'*32;self.artifact('recoveryParentReplay',m,'replay.json')
        with self.assertRaisesRegex(ValueError,'changed'):release.verify_candidate(self.write_candidate())
    def test_actual_certificate_required_and_scope_not_promoted(self):
        self.prepare();report,paths=release.verify_candidate(self.candidate);bindings={x['role']:x['sha256'] for x in report['artifacts']}
        self.assertFalse(helper.valid({},report,bindings,release.CHECKPOINT_ROOT))
        context=report['incomingRecoveryReplay'];certificate={'checkpointStateRoot':release.CHECKPOINT_ROOT,'captureManifestSha256':bindings['checkpointCaptureManifest'],'replayManifestSha256':context['manifestSha256'],'replayTranscriptSha256':'ab'*32,'chainLocalReplayVerified':True,**{k:False for k in ['publicTestCouncilFixture','consensusSealsVerified','externalFinalityVerified','operatorApprovalVerified','productionReady']}}
        council={'schema':'sora-nexus-supplied-council-replay-witness-v1','terminal':context['expectedTerminal'],'signaturesVerifiedAgainstPinnedPolicy':True,'automaticActivationObserved':True,**{k:False for k in ['publicTestCouncilGenerated','publicDynamicFixture','councilInventoryApprovalVerified','sourceProvenanceVerified','productionReady']}}
        replay={'schema':'sora-nexus-supplied-council-maintenance-replay-witness-v1','manifestSha256':context['manifestSha256'],'captureManifestSha256':bindings['checkpointCaptureManifest'],'policyCommitment':report['policy']['commitment'],'artifactSha256':context['expectedArtifacts'],'blocks':[],'automaticActivationObserved':True,'nativeExecution':True,'bridgeHaltRetained':True,'fixtureStateExpectationChecks':0,'ordinaryRuntimeBlocks':1,'replayTranscriptSha256':'ab'*32,'suppliedCouncil':council,'priorPolicyIncomingAudits':[{}],**{k:False for k in ['publicTestCouncilFixture','publicTestHashiFixture','consensusSealsVerified','operatorApprovalVerified','productionReady']}}
        block=lambda n:{'number':n,'executingCodeBlake2':'aa'*32,'independentImportRootMatches':True,'diskRestartRootMatches':True,'operationCounter':0 if n==1 else 2,'stateRoot':'0x'+'aa'*32,'resultingSpec':138 if n==3 else 136,'phase':'Complete' if n==3 else 'Auditing'}
        replay['blocks']=[block(n) for n in [1,2,3]]
        native={'block':1,**{k:True for k in ['joinedToVerifiedReplayPrefix','exactAuthoredBlockBytesMatch','completeOperationDeltaCompared','nativeTerminalSnapshotIndependentlyRestored']}}
        prior={'block':replay['blocks'][0],'executingCodeBlake2':'aa'*32,'originalPolicy':report['policy']['commitment'],'activePolicy':report['policy']['commitment'],'nativeComparison':native,**{k:False for k in ['publicTestCouncilFixture','externalFinalityVerified','operatorApprovalVerified','consensusSealsVerified','productionReady']}}
        proof={**prior,'block':replay['blocks'][1],'activePolicy':context['activePolicy']['commitment'],'consumingBlock':prior,'consumedPolicy':prior['activePolicy'],'auditCommitment':'ee'*32}
        proof['finalValidation']={'block':replay['blocks'][-1],'activePolicy':context['activePolicy']['commitment'],'auditedPolicy':proof['activePolicy'],'auditedBlock':2,'auditedCounter':2,'consumptionBlock':1,'consumptionCounter':0,'readOnlyStateRootMatches':True,'pendingIncomingAudits':0,'productionReady':False}
        certificate['finalPolicyCommitment']=context['activePolicy']['commitment'];certificate['finalStateRoot']=replay['blocks'][-1]['stateRoot']
        replay['incomingReplayBinding']=copy.deepcopy(certificate)
        replay['priorPolicyIncomingAudits']=[proof];replay['nativeOperationDerivations']=[native]
        result={'priorPolicyIncomingOperations':1,'priorPolicyReplay':certificate,'recoveryReplay':replay};self.assertTrue(helper.valid(result,report,bindings,release.CHECKPOINT_ROOT))
        # A historical audit keeps its original audited policy while final-state
        # validation binds the later policy. No rewritten audit is dispatched.
        historical=copy.deepcopy(result);p=historical['recoveryReplay']['priorPolicyIncomingAudits'][0]
        p['activePolicy']='dd'*32;p['finalValidation']['auditedPolicy']='dd'*32
        self.assertTrue(helper.valid(historical,report,bindings,release.CHECKPOINT_ROOT))
        same_policy=copy.deepcopy(historical);p=same_policy['recoveryReplay']['priorPolicyIncomingAudits'][0]
        p['consumedPolicy']=p['activePolicy'];p['consumingBlock']['activePolicy']=p['activePolicy']
        self.assertTrue(helper.valid(same_policy,report,bindings,release.CHECKPOINT_ROOT))
        for field,value in [('auditedPolicy','cc'*32),('activePolicy','cc'*32),('auditedCounter',3),('consumptionCounter',1),('pendingIncomingAudits',1),('readOnlyStateRootMatches',False),('block',replay['blocks'][0])]:
            bad=copy.deepcopy(historical);bad['recoveryReplay']['priorPolicyIncomingAudits'][0]['finalValidation'][field]=value
            self.assertFalse(helper.valid(bad,report,bindings,release.CHECKPOINT_ROOT),field)
        for field,value in [('finalStateRoot','0x'+'bb'*32),('finalPolicyCommitment','bb'*32)]:
            bad=copy.deepcopy(historical);bad['priorPolicyReplay'][field]=value;self.assertFalse(helper.valid(bad,report,bindings,release.CHECKPOINT_ROOT),field)
        for target,key,value in [('priorPolicyReplay','publicTestCouncilFixture',True),('priorPolicyReplay','replayManifestSha256','ff'*32),('recoveryReplay','policyCommitment','ff'*32),('recoveryReplay','productionReady',True)]:
            bad=copy.deepcopy(result);bad[target][key]=value;self.assertFalse(helper.valid(bad,report,bindings,release.CHECKPOINT_ROOT),key)
    def test_prior_proof_detached_block_and_public_flag_rejected(self):
        context={'activePolicy':{'commitment':'aa'*32}}
        self.assertFalse(helper.proof_joins({'blocks':[],'priorPolicyIncomingAudits':[{'block':{'number':1},'consumingBlock':{'block':{'number':0}}}]},context,'bb'*32))
    def test_default_path_rejects_unsolicited_prior_claim(self):
        self.assertTrue(helper.valid({}, {}, {}, release.CHECKPOINT_ROOT))
        self.assertFalse(helper.valid({'priorPolicyIncomingOperations':1},{},{},release.CHECKPOINT_ROOT))
    def test_missing_repair_provenance_rejected(self):
        self.prepare();del self.inventory['repair136Provenance']
        with self.assertRaisesRegex(ValueError,'provenance roles'):release.verify_candidate(self.write_candidate())
    def test_fixture_repair_provenance_rejected(self):
        self.prepare();p=json.loads((self.root/'repair136Provenance').read_text());p['fixtureOnly']=True;self.artifact('repair136Provenance',p)
        with self.assertRaisesRegex(ValueError,'fixture repair'):release.verify_candidate(self.write_candidate())
    def test_repair_original_binding_rejected(self):
        self.prepare();p=json.loads((self.root/'repair136Binding').read_text());p['originalPolicyScale']=p['policyScale'];self.artifact('repair136Binding',p)
        with self.assertRaisesRegex(ValueError,'original/active'):release.verify_candidate(self.write_candidate())
    def test_repair_build_runtime_and_log_hashes_rejected(self):
        for mutate in ['artifact','log']:
            with self.subTest(mutate=mutate):
                self.prepare();p=json.loads((self.root/'repair136BuildReceipt').read_text());p['artifacts' if mutate=='artifact' else 'logs'][next(iter(p['artifacts' if mutate=='artifact' else 'logs']))]['sha256']='ff'*32;self.artifact('repair136BuildReceipt',p)
                with self.assertRaisesRegex(ValueError,'build/artifact|raw log'):release.verify_candidate(self.write_candidate())
                # Each synthetic fixture owns its own temporary directory.
                self.setUp()
    def test_public_council_and_large_read_rejected(self):
        with self.assertRaisesRegex(ValueError,'size bound'):helper.read_bytes(Path(__file__),1)
        council=next(iter(helper.PUBLIC_COUNCIL));raw=fixtures.policy();raw=raw[:134]+council+raw[166:]
        with self.assertRaisesRegex(ValueError,'public test council'):helper.controls(raw)
    def test_retained_process_receipt_and_independent_executable_copy(self):
        directory=(self.root/'scratch');directory.mkdir();directory=directory.resolve()
        binary=self.root/'tool';binary.write_bytes(b'nonexecuted test executable')
        pin=hashlib.sha256(binary.read_bytes()).hexdigest()
        class Fake:
            pid=12345
            returncode=0
            def poll(self):return 0
            def wait(self):return 0
        def start(command,**kwargs):
            self.assertEqual(command[0],str(directory/'verifier'));self.assertEqual(kwargs['cwd'],directory)
            self.assertTrue(kwargs['start_new_session']);kwargs['stdout'].write(b'{}');return Fake()
        args=['verify-incoming-coverage','--recovery-replay','/synthetic/replay.json','--recovery-output',str(directory/'replay')]
        with mock.patch('subprocess.Popen',side_effect=start):result=helper.execute(binary,args,directory,pin)
        self.assertEqual(result.returncode,0);self.assertEqual(result.stdout,b'{}')
        receipt=json.loads((directory/'execution.json').read_text());self.assertEqual(receipt['pid'],12345)
        self.assertEqual(receipt['verifierAfterSha256'],pin);self.assertEqual((directory/'verifier').stat().st_mode&0o777,0o555)
        self.assertNotEqual(binary.stat().st_ino,(directory/'verifier').stat().st_ino)
    def test_spawn_failure_preserves_truthful_receipt(self):
        directory=self.root/'failed';directory.mkdir();directory=directory.resolve()
        binary=self.root/'tool';binary.write_bytes(b'nonexecuted')
        args=['verify-incoming-coverage','--recovery-replay','/synthetic/replay.json','--recovery-output',str(directory/'replay')]
        with mock.patch('subprocess.Popen',side_effect=OSError('synthetic spawn failure')):
            with self.assertRaisesRegex(ValueError,'process exception'):helper.execute(binary,args,directory,hashlib.sha256(binary.read_bytes()).hexdigest())
        receipt=json.loads((directory/'execution.json').read_text());self.assertIsNone(receipt['pid']);self.assertIsNone(receipt['exitCode']);self.assertIn('spawn failure',receipt['stopReason'])
    def test_saved_outputs_reject_symlink_escape(self):
        directory=self.root/'outputs';directory.mkdir();directory=directory.resolve();outside=self.root/'outside';outside.write_bytes(b'outside')
        (directory/'alias').symlink_to(outside.resolve())
        with self.assertRaisesRegex(ValueError,'alias'):helper.preserve_outputs({'artifacts':[]},{},directory,release.digest_file)
    def test_duplicate_json_rejected(self):
        self.prepare();path=self.root/'duplicate.json';path.write_text('{"a":1,"a":2}')
        with self.assertRaisesRegex(ValueError,'duplicate'):helper.read_json(path)

if __name__=='__main__':unittest.main()
