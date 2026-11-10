#!/usr/bin/env python3
"""Admit pinned additive host packages; preserve every original package and edge."""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib

HERE=Path(__file__).resolve().parent
REGISTRY='registry+https://github.com/rust-lang/crates.io-index'

def need(condition,why):
 if not condition:raise ValueError(why)
def sha(data):return hashlib.sha256(data).hexdigest()
def key(p):return (p['name'],p['version'],p.get('source',''))
def packages(lock):
 out={}
 for p in lock['package']:
  k=key(p);need(k not in out,'duplicate locked package');out[k]=p
 return out
def target(reference,table):
 pieces=reference.split(' ',2);name=pieces[0];version=pieces[1] if len(pieces)>1 else None
 source=None
 if len(pieces)==3:
  need(pieces[2].startswith('(') and pieces[2].endswith(')'),'malformed dependency source');source=pieces[2][1:-1]
 hits=[k for k in table if k[0]==name and (version is None or k[1]==version) and (source is None or k[2]==source or '#' not in source and k[2].split('#',1)[0]==source)]
 need(len(hits)==1,'missing or ambiguous locked dependency '+reference);return hits[0]
def semantic(p,table):
 return ({k:v for k,v in p.items() if k!='dependencies'},sorted(target(d,table) for d in p.get('dependencies',[])))
def allowed():
 pins=json.loads((HERE/'input-pins.json').read_text());data=(HERE/'allowed-host-packages.json').read_bytes()
 need(sha(data)==pins['allowedHostPackagesSha256'],'host package allowlist changed');return json.loads(data)
def check(before_bytes,after_bytes):
 before=tomllib.loads(before_bytes.decode());after=tomllib.loads(after_bytes.decode());a=packages(before);b=packages(after)
 need({k:v for k,v in before.items() if k!='package'}=={k:v for k,v in after.items() if k!='package'},'lock metadata changed')
 additions={key(p):p for p in allowed()}
 need(set(b)==set(a)|set(additions) and not set(a)&set(additions),'unexpected package identity added or removed')
 for k,p in a.items():need(semantic(p,a)==semantic(b[k],b),'original dependency identity/edge drift: '+repr(k))
 expected={**a,**additions}
 for k,p in additions.items():need(semantic(p,expected)==semantic(b[k],b),'host package metadata/edge differs: '+repr(k))
 return {'schema':'sora-nexus-recorder-preserved-runtime-lock-v2','originalSha256':sha(before_bytes),'resolvedSha256':sha(after_bytes),
 'originalPackagesPreserved':len(a),'originalDependencyEdgesPreserved':sum(len(p.get('dependencies',[])) for p in a.values()),
 'addedHostPackages':[list(k) for k in sorted(additions)],'allowlistSha256':sha((HERE/'allowed-host-packages.json').read_bytes()),
 'runtimePackageIdentityDrift':False,'runtimeDependencyEdgeDrift':False,'runtimeExecuted':False,'productionReady':False}

def render_candidate(before_bytes,resolved_bytes):
 original=tomllib.loads(before_bytes.decode());a=packages(original);addition_list=allowed();additions={key(p):p for p in addition_list};all_packages={**a,**additions}
 output=[]
 for i,section in enumerate(before_bytes.decode().split('[[package]]')):
  if i==0:output.append(section);continue
  p=tomllib.loads('[[package]]'+section)['package'][0]
  for dep in p.get('dependencies',[]):
   k=target(dep,a)
   try:same=target(dep,all_packages)==k
   except ValueError:same=False
   if same:continue
   replacement=k[0]+' '+k[1]
   try:need(target(replacement,all_packages)==k,'wrong qualification')
   except ValueError:replacement+=' ('+k[2]+')'
   line=' '+json.dumps(dep)+',';new=' '+json.dumps(replacement)+','
   need(section.count(line)==1,'unrecognized lock dependency line');section=section.replace(line,new)
  output.append('[[package]]'+section)
 # Keep exact package records selected by the failed offline resolution for new host-only code.
 found={}
 for section in resolved_bytes.decode().split('[[package]]')[1:]:
  p=tomllib.loads('[[package]]'+section)['package'][0]
  if key(p) in additions:
   need(p==additions[key(p)],'recorded additive host package drift');found[key(p)]='[[package]]'+section
 need(set(found)==set(additions),'missing recorded host package')
 result=''.join(output).rstrip()+'\n\n'+''.join(found[k].rstrip()+'\n\n' for k in sorted(found))
 data=result.encode();check(before_bytes,data);return data

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--original',type=Path,required=True);p.add_argument('--resolved',type=Path,required=True);a=p.parse_args();print(json.dumps(check(a.original.read_bytes(),a.resolved.read_bytes()),indent=2))
