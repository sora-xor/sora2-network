#!/usr/bin/env node
/** Immutable public fixture correction; the strict native governance check stays. */
import {readFile,writeFile,mkdir,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const require=createRequire(join(root,'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const {blake2AsHex}=require('@polkadot/util-crypto');
const assert=(ok,m)=>{if(!ok)throw Error(m);},sha=b=>createHash('sha256').update(b).digest('hex');
async function bounded(p){assert((await stat(p)).size<=64*1024**2,'file bound');const b=await readFile(p);assert(b.length<=64*1024**2,'file grew');return b;}
assert(process.argv.length===4,'Usage: extend_victim_typed_governance_fixture.mjs PRIOR_FIXTURE NEW_FIXTURE');
const[prior,output]=process.argv.slice(2).map(p=>resolve(p));
const source=await bounded(join(prior,'FIXTURE-PROVENANCE.json')),outer=JSON.parse(source);
const args=JSON.parse(await bounded(join(prior,'COMPOSE-ARGS.json')));
assert(outer.fixtureOnly===true&&outer.productionReady===false&&args.fixtureOnly===true&&args.productionReady===false,'public fixture only');
assert(args.governance===join(prior,'governance.json')&&args.semantic===join(prior,'semantic.json'),'explicit typed governance and semantic inputs required');
const governance=await bounded(args.governance),review=JSON.parse(governance);
assert(Array.isArray(review.records)&&review.records.length>0&&/^0x[0-9a-f]{64}$/.test(review.evidenceHash),'complete typed governance review required');
const semanticBytes=await bounded(args.semantic),semantic=JSON.parse(semanticBytes);
assert(semantic.governancePreservation&&typeof semantic.governancePreservation==='object','redundant unchanged waiver required');
const removed=semantic.governancePreservation;delete semantic.governancePreservation;
const after=Buffer.from(JSON.stringify(semantic,null,2)+'\n');
await mkdir(output,{recursive:false});const files=[],names=new Set();
for(const file of outer.files){
 assert(/^[a-zA-Z0-9._-]+$/.test(file.file)&&!names.has(file.file),'safe unique artifact filename');names.add(file.file);
 const original=await bounded(join(prior,file.file));assert(sha(original)===file.sha256&&original.length===file.bytes,'prior artifact checksum');
 const data=file.file==='semantic.json'?after:original;
 await writeFile(join(output,file.file),data,{flag:'wx'});files.push({...file,sha256:sha(data),bytes:data.length});
}
assert(names.has('semantic.json')&&names.has('governance.json'),'both inputs must be bound by prior provenance');
const extension={schema:'sora-nexus-public-victim-typed-governance-extension-v1',fixtureOnly:true,productionReady:false,
 priorFixture:prior,priorProvenanceSha256:sha(source),beforeSemanticSha256:sha(semanticBytes),afterSemanticSha256:sha(after),
 preservedTypedGovernanceSha256:sha(governance),removedRedundantPreservation:removed,
 purpose:'Use the complete typed governance review exclusively. No runtime check is relaxed and all other fixture inputs remain byte-identical.',sources:[]};
for(const path of ['misc/incident_recovery/prepare_victim_fixture.mjs','misc/incident_recovery/extend_victim_typed_governance_fixture.mjs','utils/incident-recovery/src/governance.rs','utils/incident-recovery/src/release_checks.rs'])extension.sources.push({path,sha256:sha(await bounded(join(root,path)))});
const extensionBytes=Buffer.from(JSON.stringify(extension,null,2)+'\n'),name='TYPED-GOVERNANCE-EXTENSION.json';assert(!names.has(name),'extension collision');
await writeFile(join(output,name),extensionBytes,{flag:'wx'});files.push({file:name,sha256:sha(extensionBytes),bytes:extensionBytes.length});
outer.files=files;outer.previousFixture={path:prior,provenanceSha256:sha(source)};outer.typedGovernanceExtension={file:name,sha256:sha(extensionBytes)};
const outerBytes=Buffer.from(JSON.stringify(outer,null,2)+'\n');await writeFile(join(output,'FIXTURE-PROVENANCE.json'),outerBytes,{flag:'wx'});
for(const[k,v]of Object.entries(args))if(typeof v==='string'&&v.startsWith(prior+'/'))args[k]=output+v.slice(prior.length);
args['evidence-commitment']=blake2AsHex(outerBytes);
await writeFile(join(output,'COMPOSE-ARGS.json'),JSON.stringify(args,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output,files:files.length,evidenceCommitment:args['evidence-commitment'],typedGovernanceOnly:true,fixtureOnly:true,productionReady:false}));
