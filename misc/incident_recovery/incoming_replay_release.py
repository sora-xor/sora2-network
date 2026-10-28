"""Bind optional authenticated recovery-parent replay; never promote production readiness."""
import hashlib,json,re,stat
from pathlib import Path
import hashi_codec as scale

ROLES={'recoveryParentReplay','incomingActivePolicyScale'}
MAX_JSON=64*1024*1024
REPLAY_TIMEOUT_SECONDS=10800
PUBLIC_COUNCIL={bytes.fromhex(x) for x in ['3d0bfb1e8c2639da9165a9b841eff0322a91e4726f5b3cfbbab33ae237248cb6','a2384b2633793ee7124bd13b0dfaab6b5c820085401e80fed28b415381f99b7c','cc75822e1a79475ca9b33bb1e9a6d331032e0b89733524d5881d4096c48649e3']}

def read_bytes(path, limit):
    scale.require(path.stat().st_size<=limit,'replay input size bound')
    with path.open('rb') as stream:raw=stream.read(limit+1)
    scale.require(len(raw)<=limit,'replay input grew')
    return raw

def read_json(path):
    return parse_json(read_bytes(path, MAX_JSON))

def parse_json(raw):
    scale.require(len(raw)<=MAX_JSON,'replay JSON size bound')
    def pairs(rows):
        out={}
        for key,value in rows:
            scale.require(key not in out,'duplicate replay JSON key');out[key]=value
        return out
    def invalid(_):raise ValueError('nonfinite replay JSON')
    return json.loads(raw,object_pairs_hook=pairs,parse_constant=invalid)

def controls(raw):
    r=scale.Reader(raw);r.take(68)
    for _ in range(r.compact()):r.take(64)
    council=[r.take(32) for _ in range(r.compact())]
    scale.require(not PUBLIC_COUNCIL.intersection(council),'public test council cannot enter release replay')
    return raw[:r.offset]

def bind(root,records,paths,original,local_file,digest_file,policy_identity,wasm_identity):
    if not (ROLES & paths.keys()):return None
    scale.require(ROLES<=paths.keys() and original is not None and {'policyScale','checkpointCaptureManifest','bootstrapWasm','holdWasm','finalWasm'}<=paths.keys(),'paired recovery replay/active policy and original release inputs required')
    active_raw=read_bytes(paths['incomingActivePolicyScale'],2*1024**2);active=policy_identity(active_raw)
    scale.require(controls(read_bytes(paths['policyScale'],2*1024**2))==controls(active_raw),'repaired policy changes checkpoint/mapping/council controls')
    source=paths['recoveryParentReplay'];manifest=read_json(source)
    allowed={'schema','checkpointHash','checkpointCapture','policy','bootstrap','hold','finalRuntime','repairs','previousGrandpaSet','freshGrandpa','suppliedCouncil','nativeOperationRecorder','nativeOperationRecordersV2','steps'}
    scale.require(set(manifest)<=allowed and manifest.get('schema')=='sora-nexus-supplied-council-maintenance-replay-v1'
        and manifest.get('checkpointHash')==scale.CHECKPOINT_HASH,'only supplied-council recovery replay accepted')
    scale.require(isinstance(manifest.get('steps'),list) and 0<len(manifest['steps'])<=4096 and isinstance(manifest.get('repairs'),list) and 0<len(manifest['repairs'])<=2,'bounded complete repair replay required')
    capture=local_file(source.parent,manifest['checkpointCapture']+'/capture-complete.json')
    scale.require(capture==paths['checkpointCaptureManifest'],'replay checkpoint capture differs from release')
    indexed={path:role for role,path in paths.items()};serial=0;aggregate=0
    def ref(value,executable=False):
        nonlocal serial,aggregate
        scale.require(isinstance(value,dict) and set(value)=={'file','sha256'} and isinstance(value['sha256'],str) and re.fullmatch('[0-9a-f]{64}',value['sha256']),'exact replay artifact reference required')
        path=local_file(source.parent,value['file']);actual=digest_file(path);scale.require(actual['sha256']==value['sha256'],'nested replay artifact changed')
        aggregate+=actual['bytes'];scale.require(aggregate<=2*1024**3,'nested replay artifact aggregate bound')
        if executable:scale.require(path.stat().st_mode&0o777==0o555 and path.stat().st_nlink==1,'native replay executable requires independent immutable pin')
        if path not in indexed:
            role=f'incomingReplayInput-{serial:05d}';serial+=1;scale.require(role not in records and len(records)<16384,'replay inventory bound/collision')
            records[role]={'role':role,**actual,'object':'objects/'+actual['sha256'],'sourcePath':path.relative_to(root).as_posix(),'executable':executable};paths[role]=path;indexed[path]=role
        elif executable:scale.require(records[indexed[path]]['executable'],'executable role is not bound executable')
        return path
    expected={}
    for field,role,spec in [('policy','policyScale',None),('bootstrap','bootstrapWasm',134),('hold','holdWasm',135),('finalRuntime','finalWasm',138)]:
        path=ref(manifest[field]);scale.require(digest_file(path)==digest_file(paths[role]),'replay original runtime/policy differs from release')
        if spec:expected[('final' if spec==138 else field)+'-'+str(spec)]=manifest[field]['sha256']
    for value in manifest['repairs']:
        path=ref(value);identity=wasm_identity(read_bytes(path,50*1024**2));spec=identity['specVersion']
        scale.require(spec in [136,137] and identity['transactionVersion']==132 and 'repair-'+str(spec) not in expected,'repair runtime identity/duplicates')
        expected['repair-'+str(spec)]=value['sha256']
        prefix='repair'+str(spec)
        required={prefix+suffix for suffix in ['PolicyScale','Binding','Provenance','BuildReceipt']}
        scale.require(required<=paths.keys(),'repair source/build provenance roles required')
        repair_raw=read_bytes(paths[prefix+'PolicyScale'],2*1024**2);repair_policy=policy_identity(repair_raw)
        scale.require(controls(repair_raw)==controls(active_raw),'repair checkpoint/mapping/council controls differ')
        binding_path=paths[prefix+'Binding'];binding=read_json(binding_path)
        required_binding={'policyScale','policyCommitment','originalPolicyScale','originalPolicyCommitment','operatorsScale','previousGrandpaSetId','firstRecoverySlot','finalWasm'}
        scale.require(set(binding)==required_binding,'repair binding fields')
        scale.require(binding['policyScale']=='0x'+repair_raw.hex() and binding['policyCommitment']=='0x'+repair_policy['commitment']
            and binding['originalPolicyScale']=='0x'+read_bytes(paths['policyScale'],2*1024**2).hex()
            and binding['originalPolicyCommitment']=='0x'+original['commitment'],'repair original/active policy binding')
        base=read_json(paths['holdBinding'])
        scale.require(all(binding[k]==base[k] for k in ['operatorsScale','previousGrandpaSetId','firstRecoverySlot']),'repair consensus/operator binding')
        scale.require(digest_file(local_file(binding_path.parent,binding['finalWasm']))==digest_file(paths['finalWasm']),'repair final138 binding')
        provenance=read_json(paths[prefix+'Provenance']);build=read_json(paths[prefix+'BuildReceipt'])
        def nonfixture(row):
            return row.get('fixtureOnly') is not True and 'fixture' not in str(row.get('schema','')).lower()
        scale.require(nonfixture(provenance) and nonfixture(build),'public fixture repair provenance cannot enter production replay')
        scale.require(provenance.get('formatVersion')==1 and provenance.get('deployedSourceTree')=='36c466398e3f2760e8ab44fdb89de796f5b0dd99'
            and provenance.get('sdk')=='polkadot-stable2603-3' and provenance.get('specVersion')==spec
            and provenance.get('transactionVersion')==132 and provenance.get('bound') is True,'repair preserved source identity')
        def pair(role):return {k:records[role][k] for k in ['sha256','blake2_256']}
        scale.require(provenance.get('bindingEvidence')=={'binding':pair(prefix+'Binding'),'policy':pair(prefix+'PolicyScale'),
            'operators':pair('operatorsScale'),'finalWasm':pair('finalWasm'),'holdWasm':None},'repair source binding hashes')
        actual=digest_file(path)
        scale.require(build.get('exitCode')==0 and build.get('status')=='passed' and build.get('specVersion')==spec
            and build.get('transactionVersion')==132 and build.get('sourceProvenance')==digest_file(paths[prefix+'Provenance'])
            and isinstance(build.get('artifacts'),dict) and list(build['artifacts'].values()).count(actual)==1,'repair actual build/artifact provenance')
        scale.require(build.get('productionReady') is False and provenance.get('releaseReady') is False,'repair provenance must not assert release approval')
        scale.require(isinstance(build.get('logs'),dict) and set(build['logs'])=={'cargo.stdout.log','cargo.stderr.log'},'repair build raw logs required')
        for name,pin in build['logs'].items():
            log=local_file(paths[prefix+'BuildReceipt'].parent,name)
            scale.require(digest_file(log)==pin,'repair build raw log binding')
            ref({'file':log.relative_to(source.parent).as_posix(),'sha256':pin['sha256']})
    original_native=manifest.get('nativeOperationRecorder');repaired_native=manifest.get('nativeOperationRecordersV2',[])
    scale.require(isinstance(repaired_native,list) and len(repaired_native)<=2 and (original_native is not None or repaired_native),'pinned native replay engines required')
    configs=([(original_native,'native-operation-executable','native-operation-engine-binding')] if original_native else [])+[(value,f'repaired-native-{i}-executable',f'repaired-native-{i}-binding') for i,value in enumerate(repaired_native)]
    for config,exe_role,binding_role in configs:
        scale.require(isinstance(config,dict) and set(config)=={'executable','engineBinding'},'native recorder config fields')
        ref(config['executable'],True);binding=read_json(ref(config['engineBinding']))
        scale.require(binding.get('schema') in ['sora-nexus-native-operation-engine-binding-v1','sora-nexus-native-operation-engine-binding-v2'] and binding.get('fixtureOnly') is False and binding.get('productionReady') is False,'public/unbound native recorder cannot enter production replay')
        expected[exe_role]=config['executable']['sha256'];expected[binding_role]=config['engineBinding']['sha256']
    for step in manifest['steps']:
        scale.require(isinstance(step,dict) and set(step)<={'slot','timestamp','authorityIndex','recovery','rejectPreviousEscapes','rejectedRecovery','expectedSpec','expectedPhase'},'fixture or unknown replay step fields')
        if step.get('recovery') is not None:ref(step['recovery'])
        rejected=step.get('rejectedRecovery',[]);scale.require(isinstance(rejected,list) and len(rejected)<=128,'rejected envelope bound')
        for value in rejected:ref(value)
    council=manifest.get('suppliedCouncil');scale.require(isinstance(council,dict) and set(council)=={'escapes','expectedTerminal'},'raw supplied council scenario required')
    terminal=council['expectedTerminal'];scale.require(isinstance(terminal,dict) and terminal.get('specVersion')==138 and terminal.get('phase')=='Complete' and terminal.get('activePolicyCommitment')=='0x'+active['commitment'] and type(terminal.get('ordinaryBlocks')) is int and terminal['ordinaryBlocks']>0,'replay terminal active policy/activation differs')
    # These are only static joins. The actual same-process runtime/certificate
    # must validate signed handover, custody/attacker controls and consumption.
    return {'activePolicy':active,'manifestSha256':records['recoveryParentReplay']['sha256'],'expectedArtifacts':expected,'expectedTerminal':terminal,'expectedBlockCount':len(manifest['steps']),'productionReady':False}

def proof_joins(replay, context, original):
    """Join diagnostics emitted by the opaque in-process certificate, not create one."""
    blocks=replay.get('blocks');proofs=replay.get('priorPolicyIncomingAudits')
    if not isinstance(blocks,list) or not isinstance(proofs,list):return False
    indexed={}
    for block in blocks:
        if not isinstance(block,dict) or type(block.get('number')) is not int or block['number'] in indexed:return False
        indexed[block['number']]=block
    original_native=replay.get('nativeOperationDerivations',[]);repaired_native=replay.get('repairedNativeOperationDerivations',[])
    if not isinstance(original_native,list) or not isinstance(repaired_native,list):return False
    native=original_native+repaired_native
    seen=set()
    for proof in proofs:
        if not isinstance(proof,dict) or not isinstance(proof.get('consumingBlock'),dict):return False
        prior=proof['consumingBlock'];audit=proof.get('block');consumption=prior.get('block')
        if not isinstance(audit,dict) or not isinstance(consumption,dict):return False
        if type(audit.get('number')) is not int or type(consumption.get('number')) is not int:return False
        if indexed.get(audit['number'])!=audit or indexed.get(consumption['number'])!=consumption or consumption['number']>=audit['number']:return False
        if proof.get('originalPolicy')!=original or prior.get('originalPolicy')!=original:return False
        if not isinstance(proof.get('activePolicy'),str) or re.fullmatch('[0-9a-f]{64}',proof['activePolicy']) is None:return False
        if proof.get('consumedPolicy')!=prior.get('activePolicy'):return False
        if proof.get('consumedPolicy')==proof.get('activePolicy')==context['activePolicy']['commitment']:return False
        commitment=proof.get('auditCommitment')
        if not isinstance(commitment,str) or re.fullmatch('[0-9a-f]{64}',commitment) is None or commitment in seen:return False
        seen.add(commitment)
        for row in [proof,prior]:
            if any(row.get(k) is not False for k in ['publicTestCouncilFixture','externalFinalityVerified','operatorApprovalVerified','consensusSealsVerified','productionReady']):return False
            block=row['block']
            if row.get('executingCodeBlake2')!=block.get('executingCodeBlake2') or any(block.get(k) is not True for k in ['independentImportRootMatches','diskRestartRootMatches']):return False
        final=proof.get('finalValidation')
        if not isinstance(final,dict) or not blocks or final.get('block')!=blocks[-1]:return False
        last=blocks[-1]
        if last.get('resultingSpec')!=138 or last.get('phase')!='Complete' or type(last.get('operationCounter')) is not int:return False
        if final.get('activePolicy')!=context['activePolicy']['commitment'] or final.get('auditedPolicy')!=proof['activePolicy']:return False
        if final.get('auditedBlock')!=audit['number'] or final.get('consumptionBlock')!=consumption['number']:return False
        if type(audit.get('operationCounter')) is not int or final.get('auditedCounter')!=audit['operationCounter'] or audit['operationCounter']>last['operationCounter']:return False
        if type(consumption.get('operationCounter')) is not int or final.get('consumptionCounter')!=consumption['operationCounter'] or consumption['operationCounter']>audit['operationCounter']:return False
        if final.get('readOnlyStateRootMatches') is not True or type(final.get('pendingIncomingAudits')) is not int or final['pendingIncomingAudits']!=0 or final.get('productionReady') is not False:return False
        if any(last.get(k) is not True for k in ['independentImportRootMatches','diskRestartRootMatches']):return False
        comparison=prior.get('nativeComparison')
        if not isinstance(comparison,dict) or comparison not in native or comparison.get('block')!=consumption['number']:return False
        if any(comparison.get(k) is not True for k in ['joinedToVerifiedReplayPrefix','exactAuthoredBlockBytesMatch','completeOperationDeltaCompared','nativeTerminalSnapshotIndependentlyRestored']):return False
    return True

def valid(result,report,bindings,checkpoint_root):
    context=report.get('incomingRecoveryReplay')
    if context is None:
        return result.get('priorPolicyIncomingOperations',0)==0 and result.get('priorPolicyReplay') is None and result.get('recoveryReplay') is None
    certificate=result.get('priorPolicyReplay');replay=result.get('recoveryReplay')
    if not isinstance(certificate,dict) or not isinstance(replay,dict):return False
    count=result.get('priorPolicyIncomingOperations');proofs=replay.get('priorPolicyIncomingAudits');council=replay.get('suppliedCouncil')
    if not isinstance(council,dict) or not isinstance(proofs,list) or type(count) is not int or count<=0 or len(proofs)!=count:return False
    return bool(certificate.get('checkpointStateRoot')==checkpoint_root and certificate.get('captureManifestSha256')==bindings['checkpointCaptureManifest']
        and certificate.get('replayManifestSha256')==context['manifestSha256']==bindings['recoveryParentReplay']
        and certificate.get('replayTranscriptSha256')==replay.get('replayTranscriptSha256') and isinstance(certificate.get('replayTranscriptSha256'),str) and re.fullmatch('[0-9a-f]{64}',certificate['replayTranscriptSha256'])
        and replay.get('incomingReplayBinding')==certificate
        and certificate.get('finalPolicyCommitment')==context['activePolicy']['commitment']
        and isinstance(replay.get('blocks'),list) and bool(replay['blocks']) and isinstance(replay['blocks'][-1],dict)
        and certificate.get('finalStateRoot')==replay['blocks'][-1].get('stateRoot')
        and isinstance(certificate.get('finalStateRoot'),str) and re.fullmatch('0x[0-9a-f]{64}',certificate['finalStateRoot'])
        and certificate.get('chainLocalReplayVerified') is True and all(certificate.get(k) is False for k in ['publicTestCouncilFixture','consensusSealsVerified','externalFinalityVerified','operatorApprovalVerified','productionReady'])
        and replay.get('schema')=='sora-nexus-supplied-council-maintenance-replay-witness-v1' and replay.get('manifestSha256')==context['manifestSha256']
        and replay.get('captureManifestSha256')==bindings['checkpointCaptureManifest'] and replay.get('policyCommitment')==report['policy']['commitment']
        and replay.get('artifactSha256')==context['expectedArtifacts'] and isinstance(replay.get('blocks'),list) and len(replay['blocks'])==context['expectedBlockCount']
        and replay.get('automaticActivationObserved') is True and replay.get('nativeExecution') is True and replay.get('bridgeHaltRetained') is True
        and all(replay.get(k) is False for k in ['publicTestCouncilFixture','publicTestHashiFixture','consensusSealsVerified','operatorApprovalVerified','productionReady'])
        and replay.get('fixtureStateExpectationChecks')==0 and replay.get('ordinaryRuntimeBlocks')==context['expectedTerminal']['ordinaryBlocks']
        and proof_joins(replay,context,report['policy']['commitment'])
        and council.get('schema')=='sora-nexus-supplied-council-replay-witness-v1' and council.get('terminal')==context['expectedTerminal']
        and council.get('signaturesVerifiedAgainstPinnedPolicy') is True and council.get('automaticActivationObserved') is True
        and all(council.get(k) is False for k in ['publicTestCouncilGenerated','publicDynamicFixture','councilInventoryApprovalVerified','sourceProvenanceVerified','productionReady']))

def preserve_outputs(report,paths,directory,digest_file):
    files=sorted(p for p in directory.rglob('*') if p.is_file());scale.require(0<len(files)<=16384,'incoming replay output count bound')
    aggregate=0
    for index,path in enumerate(files):
        scale.require(path.resolve(strict=True)==path and not path.is_symlink() and stat.S_ISREG(path.stat().st_mode),'incoming replay output alias')
        actual=digest_file(path);aggregate+=actual['bytes'];scale.require(aggregate<=8*1024**3 and len(report['artifacts'])<32768,'incoming replay output aggregate bound')
        role=f'incomingReplayOutput-{index:05d}';scale.require(role not in paths,'generated replay output role collision')
        report['artifacts'].append({'role':role,**actual,'object':'objects/'+actual['sha256'],'sourcePath':'generated-incoming-replay/'+path.relative_to(directory).as_posix(),'executable':path.name=='verifier' and path.parent==directory});paths[role]=path
    report['incomingReplayOutputDirectory']=str(directory)

def execute(binary,arguments,directory,expected_sha256):
    """Fixed coverage command with retained raw process evidence and bounded output."""
    import datetime,os,shutil,signal,subprocess,time
    scale.require(arguments[0]=='verify-incoming-coverage' and arguments.count('--recovery-replay')==1 and arguments.count('--recovery-output')==1
        and arguments[arguments.index('--recovery-output')+1]==str(directory/'replay'),'unexpected package replay command')
    target=directory/'verifier';shutil.copyfile(binary,target);target.chmod(0o555)
    with target.open('rb') as stream:actual=hashlib.file_digest(stream,'sha256').hexdigest()
    scale.require(actual==expected_sha256 and target.stat().st_nlink==1,'copied verifier identity differs')
    command=[str(target),*arguments];started=datetime.datetime.now(datetime.timezone.utc).isoformat();clock=time.monotonic();process=None;reason=None
    def write(name,value):
        with (directory/name).open('x') as stream:json.dump(value,stream,indent=2,sort_keys=True);stream.write('\n');stream.flush();os.fsync(stream.fileno())
    initial={'schema':'sora-nexus-package-incoming-replay-process-v1','command':command,'cwd':str(directory),'startedAt':started,'verifierSha256':expected_sha256,'productionReady':False}
    write('launch.json',initial)
    with (directory/'stdout.json').open('xb') as stdout,(directory/'stderr.log').open('xb') as stderr:
        try:
            process=subprocess.Popen(command,cwd=directory,stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr,start_new_session=True)
            write('started.json',{**initial,'pid':process.pid})
            while process.poll() is None:
                if time.monotonic()-clock>REPLAY_TIMEOUT_SECONDS:reason='timeout';break
                if os.fstat(stdout.fileno()).st_size>4*1024**2 or os.fstat(stderr.fileno()).st_size>4*1024**2:reason='output bound';break
                time.sleep(0.05)
        except BaseException as error:
            reason="process exception: "+str(error)
        finally:
            if process is not None:
                if process.poll() is None:
                    try:os.killpg(process.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                process.wait()
    code=process.returncode if process is not None else None
    def limited(path):
        with path.open('rb') as stream:value=stream.read(4*1024**2+1)
        scale.require(len(value)<=4*1024**2,'package replay output bound');return value
    receipt={**initial,'pid':None if process is None else process.pid,'exitCode':code,'stopReason':reason,'elapsedSeconds':time.monotonic()-clock,'finishedAt':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    for name,path in [('stdout',directory/'stdout.json'),('stderr',directory/'stderr.log')]:
        with path.open('rb') as stream:receipt[name+'Sha256']=hashlib.file_digest(stream,'sha256').hexdigest()
    try:
        with target.open('rb') as stream:receipt['verifierAfterSha256']=hashlib.file_digest(stream,'sha256').hexdigest()
    except OSError as error:
        receipt['verifierAfterSha256']=None;receipt['verifierAfterError']=str(error)
    write('execution.json',receipt)
    scale.require(receipt['verifierAfterSha256']==expected_sha256 and target.stat().st_nlink==1 and target.stat().st_mode&0o777==0o555,'copied verifier changed')
    scale.require(reason is None,'package replay stopped: '+str(reason))
    return subprocess.CompletedProcess(arguments,code,limited(directory/'stdout.json'),limited(directory/'stderr.log'))
