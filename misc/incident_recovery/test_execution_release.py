"""Synthetic evidence-format tests. No native runtime or real replay is executed."""
import copy
import hashlib
import gzip
import shutil
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import execution_release as e


def raw(value):
    return (json.dumps(value, sort_keys=True) + '\n').encode()


class MemoryArchive:
    def __init__(self):
        self.data = {}
        self.objects = {}
        self.used = set()

    def put(self, name, value):
        self.data[name] = value if isinstance(value, bytes) else raw(value)
        self.objects[name] = {'sha256': e.sha(self.data[name]), 'bytes': len(self.data[name])}
        return self.objects[name]

    def pin(self, name, expected=None, size=None):
        e.need(name in self.objects, 'missing object')
        r = self.objects[name]
        e.need(expected is None or r['sha256'] == expected, 'wrong object hash')
        e.need(size is None or r['bytes'] == size, 'wrong object size')
        self.used.add(name)
        return r

    def read(self, name, expected=None, size=None, maximum=e.MAX_JSON):
        self.pin(name, expected, size)
        e.need(len(self.data[name]) <= maximum, 'object bound')
        return self.data[name]

    def json(self, name, expected=None, maximum=e.MAX_JSON):
        return e.strict_json(self.read(name, expected, maximum=maximum), maximum)

    def close(self):
        e.need(self.used == set(self.data), 'unvisited objects')

    def replace(self, name, mutate):
        value = json.loads(self.data[name]);mutate(value);self.put(name, value)


def fixture():
    a = MemoryArchive()
    root, history, build = '/source', '/history', '/build'
    headers, previous = [], '0x' + '00' * 32
    for number in range(7):
        h = {'parentHash': previous, 'number': hex(number), 'stateRoot': '0x' + f'{number:02x}' * 32,
             'extrinsicsRoot': '0x' + 'ee' * 32, 'digest': {'logs': ['0x08'] if number == 2 else []}}
        headers.append(h)
        # Keep production bounds while synthesizing lower-numbered headers.
        previous = e.rpc_header(h)['hash']
    checkpoint = e.rpc_header(headers[6])
    candidate, policy, operators = b'\x00asm synthetic', b'policy', b'operators'
    def ref(name, value):
        return {'file': name, 'sha256': a.put(history + '/' + name, value)['sha256']}
    candidate_ref = ref('bootstrap.wasm', candidate)
    ancestry_ref = ref('ancestry.json', headers[1:])
    def capture(number, directory):
        base = history + '/' + directory
        header = e.rpc_header(headers[number]);artifacts = {}
        content = {'checkpoint-header.json': headers[number], 'parent-header.json': headers[number-1],
                   'checkpoint-block.json': {'block': {'header': headers[number], 'extrinsics': []}, 'justifications': None},
                   'runtime-version.json': {'specVersion': 133 if number == 1 else 134}, 'runtime-metadata.scale': b'metadata', 'runtime-code.wasm': b'old'}
        for name, value in content.items(): artifacts[name] = a.put(base + '/' + name, value)
        page = a.put(base + '/pages/0.gz', b'bound synthetic compressed page; not trie verified')
        m = {'schema': 'sora-nexus-checkpoint-capture-v1', 'complete': True,
             'checkpoint': {'number': number, 'hash': header['hash'], 'parentHash': header['parent'],
                            'stateRoot': header['root'], 'runtimeCodeSha256': artifacts['runtime-code.wasm']['sha256']},
             'artifacts': artifacts, 'children': [],
             'top': {'pageCount': 1, 'keyCount': 1, 'pages': [{'file':'pages/0.gz', 'sha256':page['sha256'],
                      'keyCount':1,'firstKey':'0x01','lastKey':'0x01'}]}}
        return a.put(base + '/capture-complete.json', m)
    ranges, reports = [], []
    for name, parent, numbers, boundaries in [('checkpoint',5,[6],['checkpoint']),('session',3,[4,5],['session','epoch']),('upgrade',1,[2,3],['upgrade'])]:
        cap = capture(parent, f'captures/{parent}')
        rows, refs = [], []
        for i, n in enumerate(numbers):
            h, p = e.rpc_header(headers[n]), e.rpc_header(headers[n-1])
            refs.append(ref(f'blocks/{n}.json', {'block':{'header':headers[n],'extrinsics':['0x0404']},'justifications':None}))
            spec = 133 if n == 2 else 134
            rows.append({'number':n,'hash':h['hash'],'parentHash':p['hash'],'parentStateRoot':p['root'],
                         'postStateRoot':h['root'],'deployedExecutorHash':e.blake(b'old'),
                         'candidateExecutorHash':e.blake(candidate) if spec == 134 else e.blake(b'old'),
                         'deployedSpec':spec,'substituteApplied':spec==134,'boundaries':boundaries if i==0 else [],
                         'identicalStateRoots':True,'nativeExecution':False})
        ranges.append({'name':name,'rationale':'synthetic regression','boundaries':boundaries,'parentCapture':f'captures/{parent}','blocks':refs})
        reports.append({'name':name,'rationale':'synthetic regression','boundaries':boundaries,'parentCaptureManifestSha256':cap['sha256'],'blocks':rows})
    manifest = {'schema':'sora-nexus-historical-replay-v1','checkpointHash':checkpoint['hash'],'candidate':candidate_ref,'ancestry':ancestry_ref,'ranges':ranges}
    manifest_pin = a.put(history+'/manifest.json',manifest)
    report = {'schema':'sora-nexus-historical-replay-witness-v1','checkpointHash':checkpoint['hash'],'manifestSha256':manifest_pin['sha256'],
              'candidateSha256':e.sha(candidate),'candidateBlake2':e.blake(candidate),'ancestrySha256':ancestry_ref['sha256'],
              'ranges':reports,'representativeBoundaryCoverage':True,'consensusSealsVerified':False,'productionReady':False}
    stdout_pin=a.put('/runs/history/stdout.log',report);stderr_pin=a.put('/runs/history/stderr.log',b'')
    command=['/verifier','replay-history','--manifest',history+'/manifest.json']
    plan={'schema':'sora-nexus-v15-bound-orchestration-v1','fixtureOnly':True,'productionReady':False,
          'steps':[{'id':'history','command':command,'lane':'wasm-execution','outputs':[],'cwd':root,'dependsOn':['prepare']}]}
    plan_pin=a.put('/plan.json',plan);verifier_pin=a.put('/verifier',b'native executable bytes')
    dependency=a.put('/runs/prepare/execution.json',{'exitCode':0,'status':'passed','planSha256':plan_pin['sha256']})
    before={name:dict(path=name,resolvedPath=name,**a.objects[name]) for name in ['/verifier',history+'/manifest.json']}
    started={'schema':'sora-nexus-root-selected-step-v1','step':'history','command':command,'cwd':root,'startedAt':'2026-01-01T00:00:00+00:00',
             'controllerPid':123,'planSha256':plan_pin['sha256'],'productionReady':False,'inputFilesBefore':before,
             'dependencyReceipts':[dict(path='/runs/prepare/execution.json',resolvedPath='/runs/prepare/execution.json',**dependency)],
             'sourceMutationLane':False,'outputsAbsentBefore':[],'pid':124,'processGroup':124}
    receipt=dict(started,finishedAt='2026-01-01T00:00:01+00:00',elapsedSeconds=1,exitCode=0,status='passed',inputFilesAfter=before,
                 inputChanges=[],missingOutputs=[],planUnchanged=True,stdoutSha256=stdout_pin['sha256'],stderrSha256=stderr_pin['sha256'])
    a.put('/runs/history/started.json',started);a.put('/runs/history/execution.json',receipt)
    provenance={'fixtureOnly':True,'releaseReady':False,'bound':True,'specVersion':134,'transactionVersion':131,
                'bindingEvidence':{'policy':{'sha256':e.sha(policy)},'operators':{'sha256':e.sha(operators)}}}
    provenance_pin=a.put('/build/provenance.json',provenance)
    source=a.put('/bootstrap-source/runtime/src/lib.rs',b'bootstrap source')
    sources_pin=a.put('/build/sources.json',{'runtime/src/lib.rs':source})
    a.put('/build/build.json',{'schema':'sora-nexus-fixture-wasm-build-v1','fixtureOnly':True,'productionReady':False,'status':'passed',
                             'exitCode':0,'specVersion':134,'transactionVersion':131,'artifacts':{'runtime-134.compact.compressed.wasm':a.objects[history+'/bootstrap.wasm']},
                             'sourceFingerprint':sources_pin,'sourceProvenance':provenance_pin,'source':'/bootstrap-source','logs':{}})
    native_source=a.put(root+'/runtime.rs',b'native source')
    source_files={'runtime.rs':native_source['sha256']}
    native={'command':[root+'/scripts/with_llvm_env.sh','cargo','build','--offline','--locked','-p','incident-recovery','--features','bridge-fixture'],
            'cwd':root,'pid':333,'startedAt':'2026-01-01T00:00:00+00:00','controlledEnvironment':{'SKIP_WASM_BUILD':'1'},
            'sourceBefore':source_files,'sourceAfter':source_files,'exitCode':0,'inputChanges':[],'productionReady':False,
            'log':'/native/build.log','logSha256':e.sha(b'build log')}
    a.put('/native/build.json',native);a.put('/native/started.json',{k:native[k] for k in ['command','pid','startedAt','sourceBefore','controlledEnvironment']});a.put('/native/build.log',b'build log')
    a.put('/verifier-pin.json',dict(schema='sora-nexus-public-verifier-pin-v1',fixtureOnly=True,productionReady=False,**verifier_pin,
                                 sourceFiles=source_files,actualCommandReceipts={'build':'build.json'}))
    capture(6,'checkpoint')
    cohort={}
    for role,label,data in [('bootstrapWasm',history+'/bootstrap.wasm',None),('policyScale','/policy.scale',policy),('operatorsScale','/operators.scale',operators),
                            ('signedMappingCsv','/mapping.csv',b'old,new\n'),('checkpointCaptureManifest',history+'/checkpoint/capture-complete.json',None),('bootstrapProvenance','/build/provenance.json',None)]:
        if data is not None:a.put(label,data)
        cohort[role]=dict(label=label,**a.objects[label])
    index={'schema':e.SCHEMA,'kind':'history','scope':'diagnostic-support','checkpointHash':checkpoint['hash'],'checkpointStateRoot':checkpoint['root'],
           'run':{'started':'/runs/history/started.json','completed':'/runs/history/execution.json','stdout':'/runs/history/stdout.log','stderr':'/runs/history/stderr.log',
                  'plan':'/plan.json','step':'history','verifier':'/verifier','manifest':history+'/manifest.json'},
           'cohort':cohort,'bootstrapBuild':'/build/build.json','bootstrapSources':'/build/sources.json','verifierPin':'/verifier-pin.json',
           'verifierBuildReceipt':'/native/build.json','verifierBuildStarted':'/native/started.json','verifierBuildLog':'/native/build.log','verifierSourceRoot':root,'archive':{}}
    return a,index,checkpoint


def pack(directory,a,index):
    unique={}
    for name,r in a.objects.items():unique.setdefault(r['sha256'],a.data[name])
    blob=b'';members=[]
    for digest,data in unique.items():members.append({'sha256':digest,'bytes':len(data),'offset':len(blob)});blob+=data
    (directory/'shard.bin').write_bytes(blob)
    index=copy.deepcopy(index);index['archive']={'shards':[{'role':e.PREFIX+'0000','file':'shard.bin','sha256':e.sha(blob),'bytes':len(blob),'members':members}],
                                             'objects':copy.deepcopy(a.objects)}
    p=directory/'index.json';p.write_bytes(raw(index));return p,index


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.a,self.index,checkpoint=fixture()
        self.constants=patch.multiple(e,CHECKPOINT=6,CHECKPOINT_HASH=checkpoint['hash'],CHECKPOINT_ROOT=checkpoint['root']);self.constants.start();self.addCleanup(self.constants.stop)

    def check(self):return e.check_index(self.index,self.a)
    def bad(self):
        with self.assertRaises((ValueError,KeyError,TypeError)):self.check()
    def report_mutation(self,fn):
        self.a.replace('/runs/history/stdout.log',fn)
        digest=self.a.objects['/runs/history/stdout.log']['sha256']
        self.a.replace('/runs/history/execution.json',lambda v:v.update(stdoutSha256=digest))

    def test_complete_synthetic_transcript_is_scoped(self):
        r=self.check();self.assertEqual(r['history']['blocks'],5);self.assertEqual(r['scope'],'diagnostic-support')
        self.assertFalse(r['runtimeExecutionPerformed']);self.assertFalse(r['productionReady']);self.assertFalse(r['stateTrieRecomputed'])
    def test_production_scope_rejected(self):self.index['scope']='production';self.bad()
    def test_unsupported_kind_rejected(self):self.index['kind']='maintenance';self.bad()
    def test_unknown_index_field_rejected(self):self.index['approval']=True;self.bad()
    def test_changed_cohort_policy_rejected(self):self.index['cohort']['policyScale']['sha256']='00'*32;self.bad()
    def test_false_runtime_execution_flag_rejected(self):self.report_mutation(lambda v:v['ranges'][0]['blocks'][0].update(nativeExecution=True));self.bad()
    def test_wrong_report_root_rejected(self):self.report_mutation(lambda v:v['ranges'][0]['blocks'][0].update(postStateRoot='0x'+'00'*32));self.bad()
    def test_wrong_executor_rejected(self):self.report_mutation(lambda v:v['ranges'][0]['blocks'][0].update(candidateExecutorHash='0x'+'00'*32));self.bad()
    def test_wrong_cutoff_rejected(self):self.report_mutation(lambda v:v['ranges'][0]['blocks'][0].update(substituteApplied=False));self.bad()
    def test_missing_boundary_rejected(self):self.report_mutation(lambda v:v['ranges'][1]['blocks'][0].update(boundaries=[]));self.bad()
    def test_failed_process_rejected(self):self.a.replace('/runs/history/execution.json',lambda v:v.update(exitCode=1));self.bad()
    def test_boolean_exit_rejected(self):self.a.replace('/runs/history/execution.json',lambda v:v.update(exitCode=False));self.bad()
    def test_changed_inputs_rejected(self):self.a.replace('/runs/history/execution.json',lambda v:v.update(inputChanges=['file']));self.bad()
    def test_missing_process_field_rejected(self):self.a.replace('/runs/history/execution.json',lambda v:v.pop('pid'));self.bad()
    def test_dependency_wrong_plan_rejected(self):
        self.a.replace('/runs/prepare/execution.json',lambda v:v.update(planSha256='00'*32))
        d=self.a.objects['/runs/prepare/execution.json'];fn=lambda v:v['dependencyReceipts'][0].update(d)
        self.a.replace('/runs/history/started.json',fn);self.a.replace('/runs/history/execution.json',fn);self.bad()
    def test_unbound_extra_object_rejected(self):self.a.put('/unbound',b'unbound');self.bad()
    def test_missing_capture_page_rejected(self):del self.a.objects['/history/captures/5/pages/0.gz'];self.bad()
    def test_fixture_provenance_cannot_be_relabelled(self):self.a.replace('/build/build.json',lambda v:v.update(fixtureOnly=False));self.bad()
    def test_duplicate_json_keys_rejected(self):
        with self.assertRaises(ValueError):e.strict_json(b'{"a":1,"a":2}')
    def test_noncanonical_scale_and_hex_rejected(self):
        with self.assertRaises(ValueError):e.Scale(b'\x01\x00').integer()
        with self.assertRaises(ValueError):e.unhex('0xAB')
    def test_relocated_archive_does_not_read_originals(self):
        with tempfile.TemporaryDirectory() as tmp:
            p,_=pack(Path(tmp).resolve(),self.a,self.index)
            r=e.verify_file(p,e.sha(p.read_bytes()))
            self.assertTrue(r['serializedHeadersAnchored']);self.assertFalse(r['nativeReceiptAuthenticityIndependentlyProven'])
    def test_changed_shard_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p,_=pack(Path(tmp).resolve(),self.a,self.index);(Path(tmp).resolve()/'shard.bin').write_bytes(b'changed')
            with self.assertRaises(ValueError):e.verify_file(p,e.sha(p.read_bytes()))
    def test_archive_member_gap_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p,index=pack(Path(tmp).resolve(),self.a,self.index);index['archive']['shards'][0]['members'][0]['offset']=1;p.write_bytes(raw(index))
            with self.assertRaises(ValueError):e.verify_file(p,e.sha(p.read_bytes()))
    def test_symlink_shard_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p,index=pack(Path(tmp).resolve(),self.a,self.index);base=Path(tmp).resolve();(base/'shard.bin').rename(base/'real');(base/'shard.bin').symlink_to('real')
            with self.assertRaises(ValueError):e.verify_file(p,e.sha(p.read_bytes()))
    def test_package_hook_preserves_gate_and_rejects_second_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            base=Path(tmp).resolve();p,index=pack(base,self.a,self.index)
            roles={e.ROLE:p,e.PREFIX+'0000':base/'shard.bin'}
            report={'artifacts':[{'role':role,'sha256':e.sha(path.read_bytes())} for role,path in roles.items()],
                    'gates':{'retainedHistoryStateRootReplay':{'status':'unverified'}},'productionReady':False}
            result=e.package_hook(report,roles,e.sha(Path(e.__file__).read_bytes()),base/'result')
            self.assertEqual(report['gates']['retainedHistoryStateRootReplay']['status'],'unverified')
            self.assertFalse(result['releaseGatePromoted']);self.assertTrue((base/'result/VERIFICATION-COMPLETE').is_file())
            with self.assertRaises(ValueError):e.package_hook(report,roles,e.sha(Path(e.__file__).read_bytes()),base/'result')

    def terminal_page(self, suffix=b''):
        cap='/history/captures/5/capture-complete.json'
        c=json.loads(self.a.data[cap])
        terminal={'schema':'sora-nexus-checkpoint-page-v1','checkpointHash':c['checkpoint']['hash'],
                  'terminal':True,'keyCount':0,'pageIndex':1,'startKey':'0x01','lastKey':'0x01'}
        ref=self.a.put('/history/captures/5/pages/end.gz',gzip.compress(raw(terminal)+suffix,mtime=0))
        c['top']['pageCount']=2;c['top']['pages'].append({'file':'pages/end.gz','sha256':ref['sha256'],'keyCount':0,'firstKey':None,'lastKey':'0x01'})
        digest=self.a.put(cap,c)['sha256']
        self.report_mutation(lambda v:v['ranges'][0].update(parentCaptureManifestSha256=digest))

    def test_explicit_terminal_page_is_supported(self):
        self.terminal_page();self.assertTrue(self.check()['captureObjectClosureVerified'])

    def test_terminal_page_with_row_rejected(self):
        self.terminal_page(b'{"key":"0x01","value":"0x02"}\n');self.bad()

    def test_nonfinite_number_rejected(self):
        with self.assertRaises(ValueError):e.strict_json(b'{"time":1e999}')

    def test_boolean_archive_offset_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p,index=pack(Path(tmp).resolve(),self.a,self.index)
            index['archive']['shards'][0]['members'][0]['offset']=False;p.write_bytes(raw(index))
            with self.assertRaises(ValueError):e.verify_file(p,e.sha(p.read_bytes()))

    def test_packaged_content_addressed_relocation(self):
        with tempfile.TemporaryDirectory() as tmp:
            base=Path(tmp).resolve();original=base/'original';original.mkdir()
            p,index=pack(original,self.a,self.index)
            output=base/'package';(output/'objects').mkdir(parents=True)
            artifacts=[]
            for role,source in [(e.ROLE,p),(e.PREFIX+'0000',original/'shard.bin')]:
                digest=e.sha(source.read_bytes());relative='objects/'+digest
                shutil.copyfile(source,output/relative)
                artifacts.append({'role':role,'object':relative,'sha256':digest,'bytes':source.stat().st_size})
            manifest=output/'release-manifest.json';manifest.write_bytes(raw({'format':'sora-nexus-recovery-release-v1','artifacts':artifacts}))
            shutil.rmtree(original)
            result=e.verify_package(manifest)
            self.assertTrue(result['completedTranscriptConsistent']);self.assertFalse(result['productionReady'])


if __name__=='__main__':unittest.main()
