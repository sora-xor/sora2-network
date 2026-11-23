#!/usr/bin/env node
/** Source-derived local bridge/scheduler review inputs. This does not reconcile
 * external finality, prove non-redemption, refund users or reopen any bridge. */
import {createReadStream} from 'node:fs';
import {readFile,writeFile,mkdir,lstat} from 'node:fs/promises';
import {createGunzip} from 'node:zlib';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {CHECKPOINT,MAX_PAGE_BYTES} from './capture_checkpoint.mjs';
import {streamDigest} from './checkpoint_inventory_catalog.mjs';
import {selectedRows,main as scheduler} from './prepare_scheduler_review.mjs';
import {run as cancellations} from './checkpoint_bridge_cancellations.mjs';
export const HASHI=['EthBridge.Requests','EthBridge.RequestStatuses','EthBridge.RequestApprovals',
  'EthBridge.RequestApprovers','EthBridge.RequestSubmissionHeight','EthBridge.RequestsQueue','EthBridge.AccountRequests'];
export const SIGNING=['BridgeDataSigner.Approvals','BridgeDataSigner.PendingApprovals',
  'BridgeOutboundChannel.LatestCommitment','SubstrateBridgeOutboundChannel.LatestCommitment'];
export const HISTORY=['BridgeProxy.Senders','BridgeProxy.Transactions'];
const need=(v,m)=>{if(!v)throw Error(m);},sha=b=>createHash('sha256').update(b).digest('hex');
const hx=b=>'0x'+Buffer.from(b).toString('hex'),json=v=>Buffer.from(JSON.stringify(v,null,2)+'\n');
const bytes=value=>{need(typeof value==='string'&&/^0x(?:[0-9a-f]{2})*$/.test(value),'canonical hexadecimal required');return Buffer.from(value.slice(2),'hex');};
export function compactLength(n) {
  need(Number.isSafeInteger(n)&&n>=0&&n<2**30,'bounded SCALE length required');
  const width=n<64?1:n<16384?2:4,out=Buffer.alloc(width);out.writeUIntLE(n*4+[0,1,2][Math.log2(width)],0,width);return out;
}
export function prefixReview(storage,rows,hash) {
  let commitment=hash(Buffer.from('sora-nexus-hashi-prefix-v1:'+storage)),previous=null;
  for(const row of rows) {
    need(row.storage===storage&&(previous===null||row.key>previous),'HASHI rows must be complete and strictly ordered');
    const key=bytes(row.key),valueHash=bytes(row.valueHash);need(valueHash.length===32,'HASHI value hash width');
    commitment=hash(Buffer.concat([bytes(commitment),compactLength(key.length),key,valueHash]));previous=row.key;
  }
  return {storage,records:rows.length,commitment};
}
export function checkObservedPages(manifest,observed,catalog) {
  need(observed.length===manifest.top.pages.length&&catalog.reduce((n,r)=>n+r.entries,0)===manifest.top.keyCount,'observed complete catalog differs');
  for(const [i,page]of manifest.top.pages.entries()) {
    const actual=observed[i];need(actual.file===page.file&&actual.sha256===page.sha256&&actual.keyCount===page.keyCount,'observed page identity differs');
    if(page.keyCount)need(actual.firstKey===page.firstKey&&actual.lastKey===page.lastKey,'legacy helper page hints do not match observed keys');
  }
}
export function mergeCancellations(semantic,plan) {
  need(!(semantic.bridgeCancellations??[]).length&&!(plan.controls??[]).length&&!(plan.derivedAccounts??[]).length,'conflicting bridge cancellation/semantic inputs');
  const seen=new Set();for(const row of plan.bridgeCancellations){need(!seen.has(row.callHash),'duplicate bridge call cancellation');seen.add(row.callHash);}
  return {...structuredClone(semantic),bridgeCancellations:structuredClone(plan.bridgeCancellations)};
}
export async function main(args) {
  const o={};for(let i=0;i<args.length;i+=2){need(args[i]?.startsWith('--')&&args[i+1],'expected --name value');const key=args[i].slice(2);need(!o[key],'duplicate option');o[key]=resolve(args[i+1]);}
  const required=['capture','verification','metadata-witness','trusted-mapping','prepared-reviews','historical-evidence','output'];
  need(Object.keys(o).length===required.length&&required.every(k=>o[k]),'required paths: '+required.join(', '));
  const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),pins=new Map(),capturePins=new Map();
  async function load(path,limit=64*1024**2){const info=await lstat(path);need(info.isFile()&&!info.isSymbolicLink()&&info.size<=limit,'unaliased bounded file required');const raw=await readFile(path);need(raw.length===info.size,'file changed while reading');pins.set(path,sha(raw));return raw;}
  const manifestRaw=await load(join(o.capture,'capture-complete.json')),manifest=JSON.parse(manifestRaw);
  const verificationRaw=await load(o.verification),verification=JSON.parse(verificationRaw);
  const metadataWitnessRaw=await load(o['metadata-witness']),witness=JSON.parse(metadataWitnessRaw);
  const trustedRaw=await load(o['trusted-mapping']),trusted=JSON.parse(trustedRaw);
  const generation=JSON.parse(await load(join(o['prepared-reviews'],'generation.json'))),prepared=new Map();
  need(generation.schema==='sora-nexus-automatic-checkpoint-reviews-v1'&&generation.nativeCompositionVerified===false
    &&generation.productionReady===false,'source-derived review generation scope required');
  for(const name of ['evidence.json','prefix-catalog.json','observed-page-ranges.json','semantic.json']){
    const raw=await load(join(o['prepared-reviews'],name));need(generation.outputs[name]?.sha256===sha(raw),'prepared review output hash differs');prepared.set(name,JSON.parse(raw));
  }
  const evidence=prepared.get('evidence.json'),catalog=prepared.get('prefix-catalog.json'),observed=prepared.get('observed-page-ranges.json');
  need(manifest.complete===true&&manifest.checkpoint.hash===CHECKPOINT.hash&&manifest.checkpoint.stateRoot===CHECKPOINT.stateRoot
    &&manifest.children.length===0&&verification.stateRootVerified===true&&verification.headerHashVerified===true
    &&verification.captureManifestSha256===sha(manifestRaw)&&verification.checkpointHash===CHECKPOINT.hash&&verification.stateRoot===CHECKPOINT.stateRoot,'root-verified checkpoint differs');
  need(evidence.schema==='sora-nexus-automatic-checkpoint-review-evidence-v1'&&evidence.checkpointHash===CHECKPOINT.hash
    &&evidence.stateRoot===CHECKPOINT.stateRoot&&evidence.inputSha256.checkpointManifest===sha(manifestRaw)
    &&evidence.inputSha256.verification===sha(verificationRaw)&&evidence.inputSha256['metadata-witness']===sha(metadataWitnessRaw)
    &&evidence.inputSha256['trusted-mapping']===sha(trustedRaw)&&evidence.completePrefixCatalogSha256===sha(json(catalog))
    &&evidence.observedPageRangesSha256===sha(json(observed))&&evidence.pageRangeHintsTrusted===false,'automatic review evidence joins differ');
  need(trusted.schema==='sora-nexus-trusted-csv-mapping-v1'&&trusted.authorizationPolicy==='trusted-csv-v1'&&trusted.ownershipSignaturesRequired===false
    &&trusted.unusedDestinationsVerified===true&&trusted.allEntriesClassified===true
    &&trusted.expandedMappingCommitment===evidence.expandedMappingCommitment,'trusted mapping scope differs');
  checkObservedPages(manifest,observed,catalog);
  const metadataRaw=await load(join(o.capture,'runtime-metadata.scale'));
  need(witness.wasmExecutionVerified===true&&witness.nativeExecution===false&&witness.checkpointHash===CHECKPOINT.hash
    &&witness.stateRoot===CHECKPOINT.stateRoot&&witness.metadataSha256===sha(metadataRaw)
    &&manifest.artifacts['runtime-metadata.scale'].sha256===sha(metadataRaw),'executed metadata differs');
  const dependencies=join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'),require=createRequire(dependencies);
  const {Metadata,TypeRegistry}=require('@polkadot/types'),{blake2AsU8a,xxhashAsU8a}=require('@polkadot/util-crypto');
  const hash=b=>hx(blake2AsU8a(b,256)),registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,metadataRaw);registry.setMetadata(metadata);
  const descriptors=new Map(),names=[...HASHI,...SIGNING,...HISTORY];
  for(const pallet of metadata.asLatest.toJSON().pallets)if(pallet.storage)for(const entry of pallet.storage.items){
    const name=`${pallet.name}.${entry.name}`;if(!names.includes(name))continue;
    descriptors.set(name,hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(entry.name,128)])));
  }
  need(descriptors.size===names.length,'complete native bridge metadata required');
  const byPrefix=new Map([...descriptors].map(([name,prefix])=>[prefix,name])),rows=new Map(names.map(n=>[n,[]])),pages=[];
  for(const page of observed) {
    const prefixes=[...byPrefix.keys()].filter(p=>page.keyCount&&page.firstKey<=p+'f'.repeat(1024)&&page.lastKey>=p);if(!prefixes.length)continue;
    const path=join(o.capture,page.file),digest=await streamDigest(path,MAX_PAGE_BYTES);need(digest===page.sha256,'bridge page hash differs');capturePins.set(path,digest);pages.push({file:page.file,sha256:page.sha256});
    for await(const [key,value]of selectedRows(createReadStream(path).pipe(createGunzip()),prefixes)){
      const storage=byPrefix.get(key.slice(0,66));need(storage,'unexpected selected bridge prefix');
      const list=rows.get(storage);need(!list.length||list.at(-1).key<key,'bridge keys unordered');
      list.push({storage,key,valueHash:hash(bytes(value))});need(list.length<=1000000,'bridge row bound');
    }
  }
  for(const name of names)need(rows.get(name).length===(catalog.find(r=>r.storage===name)?.entries??0),'bridge prefix completeness differs');
  const sourceNames=['misc/incident_recovery/prepare_checkpoint_bridge_reviews.mjs','misc/incident_recovery/checkpoint_bridge_cancellations.mjs','misc/incident_recovery/checkpoint_inventory_catalog.mjs',
    'misc/incident_recovery/prepare_scheduler_review.mjs','misc/incident_recovery/independent_attacker_inventory.mjs','misc/incident_recovery/capture_checkpoint.mjs',
    ...['bridge_signing','bridge_hashi','bridge_history','bridge_pending','scheduler_controls','substitution_semantics'].map(n=>`utils/incident-recovery/src/${n}.rs`),
    'vendor/sora2-common/pallets/multisig/src/lib.rs','runtime-rehearsal-async-staking-2026-10-25/validation/package.json','runtime-rehearsal-async-staking-2026-10-25/validation/package-lock.json'];
  const sources=[];for(const file of sourceNames)sources.push({file,sha256:sha(await load(join(root,file)))});
  for(const file of ['bridge-payload-evidence.json','bridge-payload-recovery-verification.json','bridge-request-recovery-source.json','bridge-archived-payload-source.json','bridge-readiness.json','verify-bridge-payload-recovery.cjs'])await load(join(o['historical-evidence'],file));
  for(const file of ['../checkpoint-verification.json','../checkpoint-metadata-witness.json'])await load(join(o.capture,file));
  const bridgeEvidence={schema:'sora-nexus-automatic-checkpoint-bridge-review-evidence-v1',checkpointHash:CHECKPOINT.hash,stateRoot:CHECKPOINT.stateRoot,
    captureManifestSha256:sha(manifestRaw),stateVerificationSha256:sha(verificationRaw),executingMetadataWitnessSha256:sha(metadataWitnessRaw),
    preparedEvidenceSha256:sha(json(evidence)),trustedMappingSha256:sha(trustedRaw),expandedMappingCommitment:trusted.expandedMappingCommitment,
    sources,pages,recordsSha256:sha(json([...rows].map(([name,records])=>({name,records})))),
    policy:{cancelKnownLocalSigning:true,cancelAllNativeOutgoing:true,quarantineNonterminalOutgoing:true,refunds:false,externalAuthorizationInvalidation:false},
    externalFinalityVerified:false,externalNonRedemptionVerified:false,humanApprovalClaimed:false,nativeCompositionVerified:false,productionReady:false};
  const evidenceHash=hash(json(bridgeEvidence)),reviews={
    'bridge-signing':{evidenceHash,cancelAllKnownSigningCommitments:true,records:SIGNING.flatMap(n=>rows.get(n))},
    'bridge-hashi':{evidenceHash,cancelAllNativeOutgoing:true,prefixes:HASHI.map(n=>prefixReview(n,rows.get(n),hash))},
    'bridge-history':{evidenceHash,cancelAllTerminalOutgoing:true,quarantineAllNonterminalOutgoing:true,records:HISTORY.flatMap(n=>rows.get(n)),preservedAnomalies:[]},
  };
  await mkdir(o.output,{recursive:false});
  await writeFile(join(o.output,'evidence.json'),json(bridgeEvidence),{flag:'wx'});
  for(const [name,review]of Object.entries(reviews))await writeFile(join(o.output,`${name}.json`),json(review),{flag:'wx'});
  const cancellationPath=join(o.output,'bridge-cancellations.json');
  const cancellationReport=await cancellations({capture:o.capture,evidence:o['historical-evidence'],dependencies,output:cancellationPath});
  const plan=JSON.parse(await readFile(cancellationPath));
  await writeFile(join(o.output,'semantic.json'),json(mergeCancellations(prepared.get('semantic.json'),plan)),{flag:'wx'});
  await scheduler(['--workspace',root,'--capture',o.capture,'--verification',o.verification,'--metadata-witness',o['metadata-witness'],'--out',join(o.output,'scheduler')]);
  for(const [path,digest]of pins)need(sha(await readFile(path))===digest,'bridge input/source changed during generation');
  for(const [path,digest]of capturePins)need(await streamDigest(path,MAX_PAGE_BYTES)===digest,'bridge capture page changed during generation');
  const outputs={};for(const file of ['evidence.json',...Object.keys(reviews).map(n=>`${n}.json`),'bridge-cancellations.json','bridge-cancellations.json.report.json','semantic.json','scheduler/evidence.json','scheduler/review.json']){
    const raw=await readFile(join(o.output,file));outputs[file]={sha256:sha(raw),blake2_256:hash(raw),bytes:raw.length};
  }
  await writeFile(join(o.output,'generation.json'),json({schema:'sora-nexus-automatic-checkpoint-bridge-reviews-v1',outputs,
    cancellationBlockers:cancellationReport.blockers,nativeCompositionVerified:false,productionReady:false,
    remaining:'Bridge owner anomalies, external authentication/finality, incoming completeness and exact-artifact rehearsal remain separate gates; no external payout or refund safety is established.'}),{flag:'wx'});
  process.stdout.write(JSON.stringify({output:o.output,cancellationBlockers:cancellationReport.blockers.length,nativeCompositionVerified:false,productionReady:false})+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main(process.argv.slice(2)).catch(error=>{console.error(error.message);process.exitCode=1;});
