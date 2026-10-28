#!/usr/bin/env python3
import datetime, hashlib, json, os, re, shlex, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];V=ROOT/'runtime-rehearsal-async-staking-2026-10-25/validation';L=V/'regression-logs';L.mkdir(exist_ok=True)
F=V/'source-clippy-start.json';REPORT=V/'native-regressions.json'
env=os.environ.copy()
for k in list(env):
 if k.startswith('REMOTE_') or k in ['RUST_LOG','WS','SNAP','REQUIRE_REMOTE','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS']:env.pop(k,None)
env.update(SKIP_WASM_BUILD='1',CARGO_TARGET_DIR=str(ROOT/'target'),LIBRARY_PATH='/opt/homebrew/opt/llvm@22/lib',LIBCLANG_PATH='/opt/homebrew/opt/llvm@22/lib',LLVM_CONFIG_PATH='/opt/homebrew/opt/llvm@22/bin/llvm-config')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def now():return datetime.datetime.now(datetime.timezone.utc).isoformat()
report={'status':'running','startedAt':now(),'sourceFingerprintSha256':sha(F),'steps':[]}
def save():REPORT.write_text(json.dumps(report,indent=2)+'\n')
def run(label,cmd,tests=True):
 log=L/(label+'.log');row={'name':label,'command':cmd,'startedAt':now()};report['steps'].append(row);save();print('Running '+label,flush=True)
 with log.open('wb') as out:p=subprocess.run(cmd,cwd=ROOT,env=env,stdout=out,stderr=subprocess.STDOUT)
 text=log.read_text(errors='replace');counts=[{'passed':int(a),'failed':int(b),'ignored':int(c)} for a,b,c in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)]
 row.update(exitCode=p.returncode,finishedAt=now(),log=str(log.relative_to(V)),logSha256=sha(log),testResults=counts);save()
 if p.returncode:raise RuntimeError(f'{label} failed ({p.returncode}); {log}')
 if tests and (not counts or sum(x['passed'] for x in counts)==0):raise RuntimeError(f'{label} silently executed no tests')
def cargo(label,pkg,extra=[]):run(label,['scripts/with_llvm_env.sh','cargo','test','--locked','-p',pkg,'--lib']+extra)
try:
 run('source-start',['python3',str(V/'source-fingerprint.py'),'--check',str(F),'--source-only','--output',str(V/'source-regressions-start-check.json')],False)
 cargo('runtime-production','framenode-runtime',['--no-default-features','--features','std,try-runtime','--','--skip','remote_'])
 cargo('runtime-local-remote-name','framenode-runtime',['--no-default-features','--features','std,try-runtime','remote_','--','--skip','remote_rehearsal_options','--skip','remote_try_runtime_upgrade_rehearsal','--skip','remote_eth_bridge_migration_rehearsal'])
 cargo('staking-async-all','pallet-staking-async')
 run('native-pinned-production',['python3',str(V/'run-native-remote.py'),'--target-dir','target','--source-fingerprint',str(F),'--execute'],False)
 assert json.loads((V/'native-remote-execution.json').read_text())['status']=='passed'
 run('source-end',['python3',str(V/'source-fingerprint.py'),'--check',str(F),'--source-only','--output',str(V/'source-regressions-end-check.json')],False)
 report['status']='passed';report['finishedAt']=now();save();print('All native regressions passed',flush=True)
except Exception as e:
 report['status']='failed';report['error']=str(e);report['finishedAt']=now();save();print(str(e),file=sys.stderr);sys.exit(1)
