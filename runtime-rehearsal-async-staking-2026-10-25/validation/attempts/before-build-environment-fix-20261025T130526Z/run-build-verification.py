#!/usr/bin/env python3
import datetime, hashlib, json, os, shutil, subprocess, sys, tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
V=ROOT/'runtime-rehearsal-async-staking-2026-10-25/validation'
L=V/'final-build-logs'; L.mkdir(exist_ok=True)
F=V/'source-clippy-start.json'
REPORT=V/'final-build-verification.json'
ENV=os.environ.copy()
for key in ['SKIP_WASM_BUILD','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS']:
 ENV.pop(key,None)
CONTROLLED={'LIBRARY_PATH':'/opt/homebrew/opt/llvm@22/lib','LIBCLANG_PATH':'/opt/homebrew/opt/llvm@22/lib','LLVM_CONFIG_PATH':'/opt/homebrew/opt/llvm@22/bin/llvm-config','CARGO_TARGET_DIR':str(ROOT/'target'),'WASM_BUILD_TYPE':'release','WASM_BUILD_NO_COLOR':'1'}
ENV.update(CONTROLLED)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def now():return datetime.datetime.now(datetime.timezone.utc).isoformat()
report={'status':'running','startedAt':now(),'workingDirectory':str(ROOT),'environment':CONTROLLED,'steps':[]}
def save():REPORT.write_text(json.dumps(report,indent=2)+'\n')
def run(label,command):
 out=L/(label+'.stdout.log');err=L/(label+'.stderr.log')
 row={'name':label,'command':command,'startedAt':now()};report['steps'].append(row);save();print('Running '+label,flush=True)
 with out.open('wb') as a,err.open('wb') as b:p=subprocess.run(command,cwd=ROOT,env=ENV,stdout=a,stderr=b)
 row.update(exitCode=p.returncode,finishedAt=now(),stdout=str(out.relative_to(V)),stdoutSha256=sha(out),stderr=str(err.relative_to(V)),stderrSha256=sha(err));save()
 if p.returncode:raise RuntimeError(f'{label} failed with {p.returncode}; inspect {err}')
 return out,err
try:
 if F.exists():
  (V/'attempts').mkdir(exist_ok=True)
  shutil.copy2(F,V/'attempts/source-before-final-lint-fixes.json')
 run('source-start',['python3',str(V/'source-fingerprint.py'),'--final','--label','final-clippy-and-production-build','--output',str(F)])
 source=json.loads(F.read_text());report['sourceFingerprint']={'path':F.name,'sha256':sha(F),'sourceSha256':source['source']['sha256'],'sourceFiles':source['source']['fileCount'],'gitHead':source['git']['head']};save()
 run('clippy-all',['housekeeping/clippy.sh','all','--message-format=json'])
 run('source-after-clippy',['python3',str(V/'source-fingerprint.py'),'--check',str(F),'--source-only','--output',str(V/'source-clippy-end-check.json')])
 run('format',['cargo','fmt','--all','--check'])
 graph,_=run('production-feature-graph',['cargo','tree','--locked','-p','framenode-runtime','--features','build-wasm-binary','-e','features','-i','framenode-runtime','--prefix','none'])
 feature_text=graph.read_text()
 assert 'framenode-runtime feature "build-wasm-binary"' in feature_text
 for forbidden in ['private-net','stage','wip','runtime-benchmarks','try-runtime']:
  assert f'framenode-runtime feature "{forbidden}"' not in feature_text,forbidden
 build=['scripts/with_llvm_env.sh','cargo','check','--locked','-p','framenode-runtime','--lib','--features','build-wasm-binary']
 run('production-wasm',build)
 manifest=ROOT/'target/debug/wbuild/framenode-runtime/Cargo.toml'
 w=tomllib.loads(manifest.read_text()); dep=w['dependencies']['wasm-project'];assert not dep.get('default-features',True)
 assert dep.get('features')==['build-wasm-binary'],dep
 wasm=ROOT/'target/debug/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm'
 candidate=V.parent/'candidate-built.compact.compressed.wasm';shutil.copy2(wasm,candidate)
 run('source-after-build',['python3',str(V/'source-fingerprint.py'),'--check',str(F),'--source-only','--output',str(V/'source-build-end-check.json')])
 toolchain=subprocess.check_output(['rustc','-Vv'],cwd=ROOT,env=ENV,text=True)
 evidence={'status':'passed','createdAt':now(),'candidateSha256':sha(candidate),'candidateBytes':candidate.stat().st_size,'candidatePath':str(candidate),'specVersion':137,'transactionVersion':133,'command':build,'buildCommand':' '.join(build),'sourceInputs':{name:sha(ROOT/name) for name in ['runtime/src/lib.rs','Cargo.lock','rust-toolchain.toml']},'environment':CONTROLLED,'sourceFingerprint':report['sourceFingerprint'],'rustc':toolchain,'wasmProjectManifestSha256':sha(manifest),'wasmProjectDependency':dep,'verificationReport':REPORT.name}
 (V/'candidate-build-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
 report['candidate']=evidence;report['status']='passed';report['finishedAt']=now();save();print(json.dumps({'status':'passed','candidate':str(candidate),'sha256':sha(candidate)}),flush=True)
except Exception as e:
 report['status']='failed';report['error']=str(e);report['finishedAt']=now();save();print(str(e),file=sys.stderr);sys.exit(1)
