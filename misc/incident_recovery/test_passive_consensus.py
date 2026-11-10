"""Synthetic admission/process-result tests; never execute node or network calls."""
import copy
import hashlib
import json
from pathlib import Path
import signal
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import passive_consensus as p
import passive_consensus_release as release


class PassiveConsensusTests(unittest.TestCase):
    def ss58(self, raw):
        payload=b'\x2a'+raw
        payload+=hashlib.blake2b(b'SS58PRE'+payload,digest_size=64).digest()[:2]
        alphabet='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
        value=int.from_bytes(payload,'big');encoded=''
        while value:
            value,digit=divmod(value,58);encoded=alphabet[digit]+encoded
        return encoded

    def target(self):
        return {'hash':'0x'+'11'*32,'number':p.CHECKPOINT+1,'stateRoot':'0x'+'22'*32,
                'codeHash':'0x'+'33'*32,'phase':'Applying','cursor':1,'operationCounter':0}

    def report(self):
        value = self.target()
        return {'schema':'sora-nexus-persisted-recovery-database-v1','checkpointHash':p.CHECKPOINT_HASH,
            'policyCommitment':'0x'+'44'*32,'bestHash':value['hash'],'finalizedHash':value['hash'],
            'checkpointNumberIndexHash':None,'checkpointHeaderPresentByHash':True,
            'checkpointImportPolicy':'0x'+'44'*32,'auxiliaries':{'synthetic':'unchanged'},
            'grandpaSetId':8,'grandpaAuthorities':[['synthetic-public-key',1]],
            'persistedFinality':[{'synthetic':'not real finality evidence'}],
            **{k:value[k] for k in ('number','stateRoot','phase','cursor','operationCounter')},
            'storage':{'0x3a636f6465':{'blake2':value['codeHash']}},'storageProofVerified':True,
            'finalityProofVerified':True,'productionReady':False,'operatorApprovalVerified':False,
            'recoveryHeaders':[{'number':value['number'],'hash':value['hash'],'parentHash':p.CHECKPOINT_HASH}],
            'observedWarp':{'pid':123,'target':value['hash'],'sdkReverified':True,'proofs':1}}

    def restart_trial(self, node=None):
        node = node if node is not None else {'sha256': 'synthetic-never-executed'}
        command = ['/synthetic/node', '--base-path', '/synthetic/warp/base', '--chain', '/synthetic/spec',
                   '--sync', 'warp', '--reserved-only', '--no-mdns']
        initial = {'pid':123,'command':command.copy(),'modeRequested':'warp','executable':node,
                   'deliberateCrash':False,'signalSent':signal.SIGTERM,'exitCode':0}
        restarted = {**initial,'pid':124,'command':command.copy()}
        value = self.target()
        admission = {'schema':'sora-nexus-passive-node-observation-v1','kind':'database-admitted',
                     'pid':124,'sequence':1,'bestHash':value['hash'],'bestNumber':value['number'],
                     'finalizedHash':value['hash'],'finalizedNumber':value['number'],
                     'checkpointNumberIndexHash':None,'checkpointHeaderPresentByHash':True,
                     'checkpointImportPolicy':'0x'+'44'*32}
        after = self.report(); after['observedWarp'] = None
        return {'initialProcess':initial,'restartProcess':restarted,'admission':admission,
                'peerObservation':{'pid':124,'method':'system_peers','result':[]},'persistedAfter':after}

    def test_warp_restart_requires_fresh_pid_and_still_absent_number_index(self):
        node = {'sha256':'synthetic-never-executed'}
        trial = self.restart_trial(node)
        p.check_warp_restart(trial,self.report(),self.target(),'0x'+'44'*32,node)
        for key in ('admission','persistedAfter'):
            for replacement in (p.CHECKPOINT_HASH,'0x'+'55'*32):
                bad=copy.deepcopy(trial);bad[key]['checkpointNumberIndexHash']=replacement
                with self.subTest(key=key,replacement=replacement),self.assertRaises(ValueError):
                    p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)
            bad=copy.deepcopy(trial);del bad[key]['checkpointNumberIndexHash']
            with self.assertRaisesRegex(ValueError,'number-index observation missing'):
                p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)
        before=self.report();before['checkpointNumberIndexHash']=p.CHECKPOINT_HASH
        with self.assertRaisesRegex(ValueError,'gap already completed'):
            p.check_warp_restart(trial,before,self.target(),'0x'+'44'*32,node)
        bad=copy.deepcopy(trial);bad['restartProcess']['pid']=123
        with self.assertRaisesRegex(ValueError,'distinct actual process'):
            p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)

    def test_warp_restart_rejects_stale_admission_peers_or_changed_consensus(self):
        node = {'sha256':'synthetic-never-executed'}
        trial = self.restart_trial(node)
        mutations = [
            ('admission','kind','database-open'),('admission','pid',123),
            ('admission','checkpointHeaderPresentByHash',False),
            ('admission','checkpointImportPolicy','0x'+'55'*32),
            ('admission','bestNumber',p.CHECKPOINT),
            ('restartProcess','modeRequested','full'),('restartProcess','executable',{}),
            ('restartProcess','exitCode',1),('peerObservation','result',[{'peerId':'unexpected'}]),
            ('peerObservation','pid',123),('persistedAfter','grandpaSetId',9),
            ('persistedAfter','grandpaAuthorities',[['another-public-key',1]]),
            ('persistedAfter','auxiliaries',{'synthetic':'changed'}),
        ]
        for section,field,value in mutations:
            bad=copy.deepcopy(trial);bad[section][field]=value
            with self.subTest(section=section,field=field),self.assertRaises(ValueError):
                p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)
        for flag in ('--reserved-nodes','--reserved-nodes=/synthetic/peer','--bootnodes'):
            bad=copy.deepcopy(trial);bad['restartProcess']['command'].extend([flag,'/synthetic/peer'])
            with self.assertRaisesRegex(ValueError,'no peers or discovery'):
                p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)
        for flag in ('--base-path','--chain','--sync'):
            bad=copy.deepcopy(trial);command=bad['restartProcess']['command'];command[command.index(flag)+1]='changed'
            with self.assertRaises(ValueError):p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)
        bad=copy.deepcopy(trial);del bad['persistedAfter']['auxiliaries']
        with self.assertRaisesRegex(ValueError,'comparison fields missing'):
            p.check_warp_restart(bad,self.report(),self.target(),'0x'+'44'*32,node)

    def test_database_admission_requires_post_guard_record_for_exact_process(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            admitted=self.restart_trial()['admission']
            opened={**admitted,'kind':'database-open','sequence':0}
            def save(rows):
                for path in root.glob('*.json'):path.unlink()
                for index,row in enumerate(rows):(root/f'{index:08}.json').write_text(json.dumps(row))
            save([opened,admitted]);self.assertEqual(p.database_admission(root,124),admitted)
            for rows in ([opened],[admitted,admitted],[opened,{**admitted,'pid':123}],
                         [{**opened,'sequence':2},admitted]):
                save(rows)
                with self.assertRaises(ValueError):p.database_admission(root,124)

    def test_warp_restart_is_a_mandatory_release_gate(self):
        self.assertIn('warpRestartBeforeGapCompletion',p.build.release.GATES)

    def test_real_warp_binding_required_even_with_mode_or_proof_claim(self):
        report = self.report()
        p.check_report(report,self.target(),'0x'+'44'*32,123,True)
        for field, replacement in [('observedWarp',None),('bestHash','0x'+'55'*32),
                ('finalizedHash','0x'+'55'*32),('storageProofVerified',False),('finalityProofVerified',False),
                ('policyCommitment','0x'+'55'*32),('productionReady',True)]:
            bad = copy.deepcopy(report); bad[field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                p.check_report(bad,self.target(),'0x'+'44'*32,123,True)
        for field in ('pid','target','sdkReverified','proofs'):
            bad = copy.deepcopy(report); del bad['observedWarp'][field]
            with self.subTest(field=field), self.assertRaises(ValueError):
                p.check_report(bad,self.target(),'0x'+'44'*32,123,True)

    def test_full_history_requires_contiguous_canonical_ancestry(self):
        for rows in ([],[{'number':p.CHECKPOINT+2,'hash':self.target()['hash'],'parentHash':p.CHECKPOINT_HASH}],
                     [{'number':p.CHECKPOINT+1,'hash':self.target()['hash'],'parentHash':'0x'+'77'*32}]):
            bad = self.report(); bad['recoveryHeaders'] = rows
            with self.assertRaises(ValueError): p.check_report(bad,self.target(),'0x'+'44'*32,full=True)

    def test_sigkill_is_observed_exit_not_an_intent_or_timeout(self):
        with patch.object(p.build,'now',return_value='synthetic'):
            self.assertTrue(p.process_receipt({'pid':123},-signal.SIGKILL,signal.SIGKILL,True)['deliberateCrash'])
            for code, sig in ((0,signal.SIGKILL),(-signal.SIGTERM,signal.SIGTERM),(1,None)):
                with self.assertRaises(ValueError): p.process_receipt({'pid':123},code,sig,True)
            self.assertFalse(p.process_receipt({'pid':123},-signal.SIGKILL,signal.SIGKILL,False)['deliberateCrash'])

    def test_full_import_trace_rejects_header_only_warp_or_wrong_process(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            rows = [{'schema':'sora-nexus-passive-node-observation-v1','pid':123,'kind':'import-result',
                     'imported':True,'origin':'NetworkInitialSync','headerOnly':False,
                     'number':n,'hash':h} for n,h in [(p.CHECKPOINT,p.CHECKPOINT_HASH),(p.CHECKPOINT+1,self.target()['hash'])]]
            def save(values):
                for i,row in enumerate(values): (root/f'{i:08}.json').write_text(json.dumps(row))
            save(rows); self.assertEqual(p.imported_history(root,123,self.report()),2)
            for key,value in [('headerOnly',True),('origin','WarpSync'),('pid',124),('imported',False)]:
                bad = copy.deepcopy(rows); bad[1][key] = value; save(bad)
                with self.assertRaises(ValueError): p.imported_history(root,123,self.report())

    def test_database_copy_rejects_links_and_mutation_without_opening_keys(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve(); src=root/'src'; src.mkdir(); (src/'data').write_bytes(b'synthetic public database')
            pins=p.inventory(src)
            with patch.object(p.shutil, 'disk_usage', return_value=SimpleNamespace(free=p.MAX_DB + p.COPY_RESERVE_MAX)):
                p.copy_tree(src,root/'copy',pins)
            self.assertEqual(p.inventory(root/'copy'),pins)
            (src/'data').write_bytes(b'changed')
            with self.assertRaises(ValueError): p.copy_tree(src,root/'changed',pins)
            (src/'link').symlink_to(src/'data')
            with self.assertRaises(ValueError): p.inventory(src)

            (src/'link').unlink(); (src/'keystore').mkdir()
            (src/'keystore'/'public-test-only').write_bytes(b'not a real key')
            with self.assertRaises(ValueError): p.inventory(src)

    def test_insufficient_copy_space_fails_before_target_or_source_scan(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();src=root/'src';src.mkdir();(src/'data').write_bytes(b'synthetic database')
            pins=p.inventory(src);target=root/'absent'/'chains'/'db';before=(src/'data').read_bytes()
            with patch.object(p.shutil, 'disk_usage', return_value=SimpleNamespace(free=0)) as usage, \
                    patch.object(p, 'inventory', side_effect=AssertionError('must reject before expensive source scan')):
                with self.assertRaisesRegex(ValueError, 'insufficient destination space'):
                    p.copy_tree(src,target,pins)
            usage.assert_called_once_with(root)
            self.assertFalse((root/'absent').exists())
            self.assertEqual((src/'data').read_bytes(),before)

    def test_copy_space_is_rechecked_after_scan_and_uses_bounded_reserve(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();src=root/'src';src.mkdir();(src/'data').write_bytes(b'synthetic database')
            pins=p.inventory(src);target=root/'copy'
            with patch.object(p.shutil, 'disk_usage', side_effect=[
                    SimpleNamespace(free=p.MAX_DB+p.COPY_RESERVE_MAX),SimpleNamespace(free=0)]):
                with self.assertRaisesRegex(ValueError,'insufficient destination space'):
                    p.copy_tree(src,target,pins)
            self.assertFalse(target.exists())
            huge={'data':{'bytes':p.MAX_DB}}
            with patch.object(p.shutil, 'disk_usage', return_value=SimpleNamespace(free=p.MAX_DB+p.COPY_RESERVE_MAX)):
                budget=p.copy_space(target,huge)
            self.assertEqual(budget['requiredBytes'],p.MAX_DB)
            self.assertEqual(budget['reserveBytes'],p.COPY_RESERVE_MAX)
            for invalid in ({'data':{'bytes':-1}},{'data':{'bytes':True}},{'data':{'bytes':p.MAX_DB+1}}):
                with self.assertRaises(ValueError):p.copy_space(target,invalid)

    def test_rpc_capture_cannot_be_admitted_as_a_client_database(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();capture=root/'capture';capture.mkdir()
            (capture/'capture-complete.json').write_text(json.dumps({'schema':'sora-nexus-checkpoint-capture-v1',
                'complete':True,'consensusProofVerified':False,'historyComplete':False}))
            row={'path':'capture','inventory':{'path':'not-yet-read.json','sha256':'0'*64}}
            with patch.object(p,'inventory',side_effect=AssertionError('must reject before scanning pages')):
                with self.assertRaisesRegex(ValueError,'RPC captures are not client databases'):
                    p.db_input(root,row)

    def test_rocksdb_shape_only_admits_one_existing_unaliased_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();current=root/'CURRENT'
            manifest=root/'MANIFEST-000001';manifest.write_bytes(b'synthetic structural marker, not valid database evidence')
            current.write_bytes(b'MANIFEST-000001\n');p.require_rocksdb_shape(root)
            for raw in [b'../MANIFEST-000001\n',b'/MANIFEST-000001\n',b'MANIFEST-000001',
                        b'MANIFEST-000001\nMANIFEST-000001\n',b'x'*129,b'MANIFEST-999999\n']:
                current.write_bytes(raw)
                with self.subTest(raw=raw),self.assertRaises(ValueError):p.require_rocksdb_shape(root)
            current.write_bytes(b'MANIFEST-000001\n');manifest.unlink();manifest.symlink_to(current)
            with self.assertRaises(ValueError):p.require_rocksdb_shape(root)
            manifest.unlink();manifest.write_bytes(b'')
            with self.assertRaises(ValueError):p.require_rocksdb_shape(root)

    def test_native_route_and_fixed_stage_schema_reject_remote_or_commands(self):
        value={'schema':p.SCHEMA,'route':'native-loopback','ports':list(range(30000,30010)),
            'target':self.target(),'crashStages':[{'providerDatabase':{},'target':self.target()}]}
        value.update({k:{} for k in ('node','chainSpec','recoveryConfig','bootstrapWasm','sourceInventory','predecessorDatabase','providerDatabase')})
        p.validate_manifest(value)
        for key,bad_value in [('route','docker-remote'),('ports',[30000]*10),('command',['sh','-c','false']),('crashStages',[])]:
            bad=copy.deepcopy(value);bad[key]=bad_value
            with self.assertRaises(ValueError):p.validate_manifest(bad)

    def test_canonical_ss58_public_keys_rejected_in_active_and_approved_roles(self):
        safe=self.ss58(bytes.fromhex('42'*32))
        p.reject_public_keys({'grandpaAuthorities':[[safe,1]],'approvedGrandpaKeys':[safe]})
        for key in p.build.PUBLIC_SIGNING_KEYS:
            known=self.ss58(bytes.fromhex(key))
            for config in ({'grandpaAuthorities':[[known,1]],'approvedGrandpaKeys':[safe]},
                           {'grandpaAuthorities':[[safe,1]],'approvedGrandpaKeys':[known]}):
                with self.subTest(key=key),self.assertRaises(ValueError):p.reject_public_keys(config)

    def test_runtime_code_and_recovery_counter_mismatch_rejected(self):
        for field,value in [('phase','Complete'),('cursor',2),('operationCounter',1),('number',p.CHECKPOINT+2),('stateRoot','0x'+'99'*32)]:
            bad=self.report();bad[field]=value
            with self.assertRaises(ValueError):p.check_report(bad,self.target(),'0x'+'44'*32)
        bad=self.report();bad['storage']['0x3a636f6465']['blake2']='0x'+'99'*32
        with self.assertRaises(ValueError):p.check_report(bad,self.target(),'0x'+'44'*32)

    def test_release_hook_rejects_source_change_before_any_harness(self):
        with patch.object(p,'Harness',side_effect=AssertionError('must not execute')):
            with self.assertRaises(ValueError):
                release.package_hook({}, {}, Path('/unused'), '0'*64, Path('/unused'))

    def test_release_hook_rejects_wrong_release_node_policy_and_final_code(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve()
            fields={}
            paths={}
            for role,name in [('nodeBinary','node'),('chainSpec','chainSpec'),('bootstrapWasm','bootstrapWasm'),('finalWasm','final')]:
                path=root/name;path.write_bytes(('public synthetic '+name).encode());paths[role]=path
                if name!='final':fields[name]={'path':name,'sha256':p.digest(path)['sha256']}
            config={'policyCommitment':'0x'+'44'*32,'finalCodeHash':'0x'+p.digest(paths['finalWasm'])['blake2_256']}
            (root/'config').write_text(json.dumps(config));fields['recoveryConfig']={'path':'config','sha256':p.digest(root/'config')['sha256']}
            end=self.target();end.update(phase='Complete',codeHash=config['finalCodeHash'])
            manifest={'schema':p.SCHEMA,'route':'native-loopback',**fields,'sourceInventory':{},
                'predecessorDatabase':{},'providerDatabase':{},'target':end,
                'crashStages':[{'providerDatabase':{},'target':end}],'ports':list(range(30000,30010))}
            file=root/'input.json';file.write_text(json.dumps(manifest))
            report={'policy':{'commitment':'44'*32}}
            release.validate_release(report,paths,file)
            with self.assertRaises(ValueError):release.validate_release({'policy':{'commitment':'55'*32}},paths,file)
            bad=root/'wrong-node';bad.write_bytes(b'not the release node')
            with self.assertRaises(ValueError):release.validate_release(report,{**paths,'nodeBinary':bad},file)
            changed=copy.deepcopy(manifest);changed['target']['phase']='Hold';file.write_text(json.dumps(changed))
            with self.assertRaises(ValueError):release.validate_release(report,paths,file)

    def test_failed_final_registration_does_not_mutate_package_paths_or_gates(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve(); manifest=root/'input.json';manifest.write_text('{}')
            node=root/'node';node.write_bytes(b'synthetic node never executed')
            spec=root/'spec';spec.write_bytes(b'synthetic spec')
            paths={'nodeBinary':node,'chainSpec':spec};original_paths=paths.copy()
            report={'artifacts':[],'gates':{'fullAndWarpSynchronization':{'status':'unverified'}},'productionReady':False}
            original_report=copy.deepcopy(report)
            output=root/'out';output.mkdir();completion=output/'completion.json';completion.write_text('{}')
            result={'node':p.digest(node),'chainSpec':p.digest(spec),'policyCommitment':'0x'+'44'*32,
                'fullAndWarpSynchronization':{'full':self.report(),'warp':self.report(),
                    'warpRestartBeforeGapCompletion':self.restart_trial(p.digest(node))},
                'committedBoundaryCrashes':[],'allRecoveryBlocksKilledAfterCommit':False}
            class SyntheticHarness:
                def __init__(self,*_):
                    self.output=output;self.pins={str(completion):p.digest(completion)}
                def run(self):return result
                def originals(self):node.write_bytes(b'mutation immediately before registration')
            with patch.object(release,'validate_release',return_value={'target':self.target()}),patch.object(p,'Harness',SyntheticHarness):
                with self.assertRaisesRegex(ValueError,'before registration'):
                    release.package_hook(report,paths,manifest,release.source_binding(),output)
            self.assertEqual(paths,original_paths)
            self.assertEqual(report,original_report)

    def test_release_hook_rejects_missing_warp_restart_before_registering_gates(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();manifest=root/'input.json';manifest.write_text('{}')
            node=root/'node';node.write_bytes(b'synthetic node never executed')
            spec=root/'spec';spec.write_bytes(b'synthetic spec')
            paths={'nodeBinary':node,'chainSpec':spec};original_paths=paths.copy()
            report={'artifacts':[],'gates':{},'productionReady':False};original_report=copy.deepcopy(report)
            result={'node':p.digest(node),'chainSpec':p.digest(spec),'policyCommitment':'0x'+'44'*32,
                'fullAndWarpSynchronization':{'full':self.report(),'warp':self.report()}}
            class SyntheticHarness:
                def __init__(self,*_):pass
                def run(self):return result
            with patch.object(release,'validate_release',return_value={'target':self.target()}),patch.object(p,'Harness',SyntheticHarness):
                with self.assertRaisesRegex(ValueError,'restart before gap completion is mandatory'):
                    release.package_hook(report,paths,manifest,release.source_binding(),root/'unused')
            self.assertEqual(paths,original_paths);self.assertEqual(report,original_report)

    def test_failed_completion_digest_does_not_register_partial_gate(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();manifest=root/'input.json';manifest.write_text('{}')
            node=root/'node';node.write_bytes(b'synthetic node never executed')
            spec=root/'spec';spec.write_bytes(b'synthetic spec')
            paths={'nodeBinary':node,'chainSpec':spec};original_paths=paths.copy()
            report={'artifacts':[],'gates':{'fullAndWarpSynchronization':{'status':'unverified'}},'productionReady':False}
            original_report=copy.deepcopy(report)
            output=root/'out';output.mkdir();completion=output/'completion.json';completion.write_text('{}')
            result={'node':p.digest(node),'chainSpec':p.digest(spec),'policyCommitment':'0x'+'44'*32,
                'fullAndWarpSynchronization':{'full':self.report(),'warp':self.report(),
                    'warpRestartBeforeGapCompletion':self.restart_trial(p.digest(node))},
                'committedBoundaryCrashes':[],'allRecoveryBlocksKilledAfterCommit':False}
            pins={str(completion):p.digest(completion)};original_digest=p.digest;reads=0
            class SyntheticHarness:
                def __init__(self,*_):self.output=output;self.pins=pins
                def run(self):return result
                def originals(self):pass
            def failing_digest(path):
                nonlocal reads
                if path==completion:
                    reads+=1
                    if reads==2:raise ValueError('completion disappeared during gate preparation')
                return original_digest(path)
            with patch.object(release,'validate_release',return_value={'target':self.target()}),patch.object(p,'Harness',SyntheticHarness),patch.object(p,'digest',failing_digest):
                with self.assertRaisesRegex(ValueError,'completion disappeared'):
                    release.package_hook(report,paths,manifest,release.source_binding(),output)
            self.assertEqual(paths,original_paths)
            self.assertEqual(report,original_report)


if __name__ == '__main__': unittest.main()
