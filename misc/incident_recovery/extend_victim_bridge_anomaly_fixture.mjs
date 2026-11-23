#!/usr/bin/env node
/** Immutable PUBLIC-FIXTURE-ONLY extension. No new victim authorization. */
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
assert(process.argv.length===5,'Usage: extend_victim_bridge_anomaly_fixture.mjs PRIOR_FIXTURE ANOMALY_REVIEW_DIRECTORY NEW_FIXTURE');
const[prior,anomalies,output]=process.argv.slice(2).map(p=>resolve(p));
const source=await bounded(join(prior,'FIXTURE-PROVENANCE.json')),outer=JSON.parse(source);
assert(outer.fixtureOnly===true&&outer.productionReady===false,'public fixture only');
const review=await bounded(join(anomalies,'review.json')),evidence=await bounded(join(anomalies,'evidence.json'));
const parsed=JSON.parse(review),ev=JSON.parse(evidence);
assert(parsed.evidenceHash===blake2AsHex(evidence)&&parsed.preservedAnomalies?.length===ev.anomalies?.length&&ev.fixtureOnly===true&&!ev.productionReady,'exact anomaly evidence');
const oldBytes=await bounded(join(prior,'bridge-history.json'));
assert(sha(oldBytes)===ev.baseReviewSha256,'anomaly review must extend this exact prior inventory');
await mkdir(output,{recursive:false});const files=[],names=new Set();
for(const file of outer.files){
 assert(/^[a-zA-Z0-9._-]+$/.test(file.file)&&!names.has(file.file),'safe unique artifact filename');names.add(file.file);
 const original=await bounded(join(prior,file.file));assert(sha(original)===file.sha256,'prior artifact checksum');
 const data=file.file==='bridge-history.json'?review:original;
 await writeFile(join(output,file.file),data,{flag:'wx'});files.push({...file,...(file.file==='bridge-history.json'?{source:join(anomalies,'review.json')}:{}),sha256:sha(data),bytes:data.length});
}
const evidenceName='bridge-anomaly-evidence.json';assert(!names.has(evidenceName),'extension collision');
await writeFile(join(output,evidenceName),evidence,{flag:'wx'});files.push({file:evidenceName,sha256:sha(evidence),bytes:evidence.length});
const extension={schema:'sora-nexus-public-victim-bridge-anomaly-extension-v1',fixtureOnly:true,productionReady:false,
 priorFixture:prior,priorProvenanceSha256:sha(source),anomalyEvidenceSha256:sha(evidence),anomalyEvidenceHash:parsed.evidenceHash,
 unrelatedUnresolvedAnomalies:parsed.preservedAnomalies.length,
 purpose:'Preserve unrelated ambiguous checkpoint records and install no-refund quarantine markers. No financial reinterpretation, counterpart reconciliation or victim authorization.',
 sources:[]};
for(const path of ['utils/incident-recovery/src/bridge_history.rs','misc/incident_recovery/prepare_bridge_history_anomalies.mjs','misc/incident_recovery/extend_victim_bridge_anomaly_fixture.mjs'])extension.sources.push({path,sha256:sha(await bounded(join(root,path)))});
const extensionBytes=Buffer.from(JSON.stringify(extension,null,2)+'\n');
await writeFile(join(output,'BRIDGE-ANOMALY-EXTENSION.json'),extensionBytes,{flag:'wx'});files.push({file:'BRIDGE-ANOMALY-EXTENSION.json',sha256:sha(extensionBytes),bytes:extensionBytes.length});
outer.files=files;outer.previousFixture={path:prior,provenanceSha256:sha(source)};
outer.bridgeAnomalyExtension={file:'BRIDGE-ANOMALY-EXTENSION.json',sha256:sha(extensionBytes)};
outer.outstanding.push('Reconcile the four byte-preserved ambiguous bridge records; native containment does not establish their financial meaning');
const outerBytes=Buffer.from(JSON.stringify(outer,null,2)+'\n');await writeFile(join(output,'FIXTURE-PROVENANCE.json'),outerBytes,{flag:'wx'});
const args=JSON.parse(await bounded(join(prior,'COMPOSE-ARGS.json')));
for(const[k,v]of Object.entries(args))if(typeof v==='string'&&v.startsWith(prior+'/'))args[k]=output+v.slice(prior.length);
args['evidence-commitment']=blake2AsHex(outerBytes);
await writeFile(join(output,'COMPOSE-ARGS.json'),JSON.stringify(args,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output,files:files.length,evidenceCommitment:args['evidence-commitment'],unresolvedAnomalies:parsed.preservedAnomalies.length,fixtureOnly:true,productionReady:false}));
