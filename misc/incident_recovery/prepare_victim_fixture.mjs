#!/usr/bin/env node
/** Public deterministic diagnostic ONLY. Never a victim authorization or release. */
import {createReadStream, readFileSync, writeFileSync, mkdirSync, copyFileSync, statSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {createRequire} from 'node:module';
import {resolve, join, dirname, relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {selectedRows} from './prepare_scheduler_review.mjs';
const repo=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const output=resolve(process.argv[2]??join(repo,'artifacts/recovery-28191817/fixtures/victim-v1'));
if(process.argv.length>3)throw Error('Usage: prepare_victim_fixture.mjs NEW_OUTPUT_DIRECTORY');
const capture=join(repo,'artifacts/recovery-28191817/checkpoint');
const require=createRequire(join(repo,'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const {TypeRegistry,Metadata}=require('@polkadot/types');
const {xxhashAsU8a,blake2AsHex,blake2AsU8a,ed25519PairFromSeed,ed25519Sign,ed25519Verify,cryptoWaitReady,decodeAddress,encodeAddress}=require('@polkadot/util-crypto');
await cryptoWaitReady();
const hx=b=>'0x'+Buffer.from(b).toString('hex'),bytes=s=>Buffer.from(s.slice(2),'hex'),sha=b=>createHash('sha256').update(b).digest('hex');
const bounded=(p,n=64*1024**2)=>{if(statSync(p).size>n)throw Error('input size bound '+p);const b=readFileSync(p);if(b.length>n)throw Error('input grew '+p);return b;};
const load=p=>JSON.parse(bounded(p));
const checkpoint='0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0';
const genesis='0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5';
const old='0x021412e1da26502b95d6a9903d3f65284e183d2c35df061ddabf9e87a9c65868';
const manifestBytes=bounded(join(capture,'capture-complete.json')),captureManifest=JSON.parse(manifestBytes);
const verified=load(join(capture,'../checkpoint-verification.json')),metadataWitness=load(join(capture,'../checkpoint-metadata-witness.json'));
if(!captureManifest.complete||captureManifest.checkpoint.hash!==checkpoint||verified.captureManifestSha256!==sha(manifestBytes)||!verified.stateRootVerified||!metadataWitness.wasmExecutionVerified)throw Error('authenticated complete checkpoint evidence required');
const metadataBytes=bounded(join(capture,'runtime-metadata.scale'));
if(sha(metadataBytes)!==metadataWitness.metadataSha256)throw Error('executed metadata witness mismatch');
const registry=new TypeRegistry(),metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);const mj=metadata.asLatest.toJSON();
const desc=new Map();for(const p of mj.pallets)if(p.storage)for(const e of p.storage.items)desc.set(p.name+'.'+e.name,{prefix:hx(Buffer.concat([xxhashAsU8a(p.storage.prefix,128),xxhashAsU8a(e.name,128)])),entry:e});
const did=n=>{const d=desc.get(n);if(!d)throw Error('missing metadata '+n);return d;};
const seenPages=new Map(),cache=new Map();
async function raw(prefix){if(cache.has(prefix))return cache.get(prefix);const result=[];for(const page of captureManifest.top.pages){if(!page.keyCount||page.lastKey<prefix||page.firstKey>prefix+'f'.repeat(1024))continue;if(!/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file))throw Error('capture path');const p=join(capture,page.file);if(!seenPages.has(page.file)){const hash=createHash('sha256');for await(const chunk of createReadStream(p))hash.update(chunk);if(hash.digest('hex')!==page.sha256)throw Error('capture page digest');seenPages.set(page.file,page.sha256);}for await(const row of selectedRows(createReadStream(p).pipe(createGunzip()),[prefix])){result.push(row);if(result.length>100000)throw Error('review prefix row bound');}}cache.set(prefix,result);return result;}
const rows=n=>raw(did(n).prefix);
function decoded(n,b){const e=did(n).entry;const v=registry.createTypeUnsafe('Lookup'+(e.type.map?.value??e.type.plain),[bytes(b)]);if(hx(v.toU8a())!==b)throw Error('exact codec '+n);return v.toJSON();}
const aid=a=>hx(decodeAddress(a));
const key=(n,a)=>{const d=did(n),h=d.entry.type.map.hashers[0];if(d.entry.type.map.hashers.length!==1||!['Blake2_128Concat','Twox64Concat'].includes(h))throw Error('account key codec '+n);return d.prefix+hx(h==='Blake2_128Concat'?blake2AsU8a(bytes(a),128):xxhashAsU8a(bytes(a),64)).slice(2)+a.slice(2);};
async function record(n,k){const v=(await raw(k)).find(([x])=>x===k)?.[1];if(v===undefined)throw Error('missing required record '+n+' '+k);return{storage:n,key:k,valueHash:blake2AsHex(bytes(v))};}
const publicSeed=blake2AsU8a(Buffer.from('SORA Nexus PUBLIC NEVER DEPLOY victim replacement fixture v1'),256),pair=ed25519PairFromSeed(publicSeed),newOwner=hx(pair.publicKey);
const domain=Buffer.from('sora-nexus/replacement-ownership/v1');if(domain.length>=64)throw Error('compact domain assumption');
const ownershipPayload=Buffer.concat([Buffer.from([domain.length<<2]),domain,bytes(genesis),bytes(checkpoint),bytes(old),bytes(newOwner)]);
const sig=ed25519Sign(ownershipPayload,pair);if(!ed25519Verify(ownershipPayload,sig,pair.publicKey))throw Error('fixture signature failed');
const sourceFiles=['utils/incident-recovery/src/elections.rs','utils/incident-recovery/src/offences.rs','utils/incident-recovery/src/bridge_history.rs','utils/incident-recovery/src/staking_controls.rs','utils/incident-recovery/src/staking_roles.rs','utils/incident-recovery/src/staking_history.rs','utils/incident-recovery/src/referral.rs','utils/incident-recovery/src/substitution_semantics.rs','utils/incident-recovery/src/inventory.rs','misc/incident_recovery/prepare_victim_fixture.mjs'];
const evidence={schema:'sora-nexus-public-victim-fixture-evidence-v1',fixtureOnly:true,productionReady:false,checkpoint,old,newOwner,publicTestSeed:hx(publicSeed),newKeyOwnershipPurpose:'Proves only this published test key controls its synthetic destination; it does not authorize replacing the observed real account.',captureManifestSha256:sha(manifestBytes),metadataSha256:sha(metadataBytes),sources:sourceFiles.map(path=>({path,sha256:sha(bounded(join(repo,path)))})),excludedGates:['actual victim authorization','pre-compromise control-history approval','operator inventory approval','recovered council approval','complete derived-account attestation','attacker asset settlement','bridge counterpart reconciliation','release approval']};
const evidenceBytes=JSON.stringify(evidence,null,2)+'\n',evidenceHash=blake2AsHex(Buffer.from(evidenceBytes));
mkdirSync(output);
const outputs=[];function save(name,value){const b=typeof value==='string'?Buffer.from(value):Buffer.from(JSON.stringify(value,null,2)+'\n');writeFileSync(join(output,name),b,{flag:'wx'});outputs.push({file:name,sha256:sha(b),bytes:b.length});}
function copy(name,source){const b=bounded(source);writeFileSync(join(output,name),b,{flag:'wx'});outputs.push({file:name,sha256:sha(b),bytes:b.length,source:relative(repo,source)});}
save('fixture-evidence.json',evidenceBytes);
save('mapping.csv','"old account address","new account address"\n"'+old+'","'+newOwner+'"\n');
save('ownership-proofs.json',[{old,new:newOwner,signature:hx(Buffer.concat([Buffer.from([0]),sig]))}]);
save('ownership-payload.json',{old,new:newOwner,payload:hx(ownershipPayload),publicTestSeed:hx(publicSeed),ss58Old:encodeAddress(bytes(old),69),ss58New:encodeAddress(pair.publicKey,69),fixtureOnly:true});
const options=load(join(capture,'../checkpoint-inventory-options-v3.json')),inventory=load(join(capture,'../checkpoint-inventory-verified-v3.json'));
if(inventory.topEntries!==verified.topKeys||!inventory.allEntriesClassified)throw Error('need complete baseline codec inventory');
options.watchedAccounts=[old,newOwner];
const generic=new Set(['System.Account','Balances.Locks','Balances.Holds','Balances.Freezes','Balances.Reserves','Tokens.Accounts']);
options.dispositions=inventory.prefixes.map(p=>({trie:p.trie,storage:p.storage,action:generic.has(p.storage)?'mutable_ownership':'retain_historical',reason:generic.has(p.storage)?'PUBLIC FIXTURE ONLY: exact typed owner-key relocation; complete Rust inventory and financial certificates mandatory.':'PUBLIC FIXTURE ONLY: retain exact checkpoint bytes for rows not replaced by a typed adapter. Any mapped mutable reference without a specific adapter must fail; this is not a production authorization or claim that current economic state is immutable.',evidenceHash}));
save('inventory.json',options);
const semantic=load(join(capture,'../checkpoint-bridge-cancellations.json'));
semantic.controls.push({storage:'Staking.Payee',key:key('Staking.Payee',old),expectedValueHash:(await record('Staking.Payee',key('Staking.Payee',old))).valueHash,action:'default_staking_payee',reviewEvidenceHash:evidenceHash});
// The complete typed governance review below owns roster/prime preservation.
// An unchanged-state waiver is a mutually exclusive path, not extra assurance.
delete semantic.governancePreservation;
save('semantic.json',semantic);
save('economic.json',{attackers:['0x64650caa38ca952fc214cbb38cb625d779d1387b52910e664012cbd7d7593a7d'],protectedOwners:[old,newOwner],directBurns:[],extinguishUnissued:[],quarantine:[]});
const bonded=(await raw(key('Staking.Bonded',old))).find(([k])=>k===key('Staking.Bonded',old));if(!bonded)throw Error('chosen victim missing bond');const controller=aid(decoded('Staking.Bonded',bonded[1]));
save('staking-controls.json',{evidenceHash,pairs:[{stash:old,controller,bondedHash:blake2AsHex(bytes(bonded[1])),ledgerHash:(await record('Staking.Ledger',key('Staking.Ledger',controller))).valueHash}]});
const forward=await rows('Referrals.Referrers'),reverse=await rows('Referrals.Referrals'),refRecords=new Map(),affected=new Set();
for(const[k,v]of forward){const owner='0x'+k.slice(-64),referrer=aid(decoded('Referrals.Referrers',v));if(owner!==old&&referrer!==old)continue;refRecords.set(k,{storage:'Referrals.Referrers',key:k,valueHash:blake2AsHex(bytes(v))});affected.add(owner);affected.add(referrer);}
for(const[k,v]of reverse){const owner='0x'+k.slice(-64),targets=decoded('Referrals.Referrals',v).map(aid);if(owner!==old&&!targets.includes(old))continue;refRecords.set(k,{storage:'Referrals.Referrals',key:k,valueHash:blake2AsHex(bytes(v))});}
for(const owner of affected){const r=await record('System.Account',key('System.Account',owner));refRecords.set(r.key,r);}
save('referral.json',{evidenceHash,records:[...refRecords.values()].sort((a,b)=>a.key.localeCompare(b.key))});
const govNames=['Council','TechnicalCommittee'].flatMap(p=>['Members','Prime','Proposals','ProposalCount','ProposalOf','Voting','CostOf'].map(i=>p+'.'+i)).concat(['TechnicalMembership.Members','TechnicalMembership.Prime']);
const govRecords=[];for(const n of govNames){const rs=await rows(n);if(!did(n).entry.type.map&&!rs.length)govRecords.push({storage:n,key:did(n).prefix,valueHash:null});for(const[k,v]of rs)govRecords.push({storage:n,key:k,valueHash:blake2AsHex(bytes(v))});}
save('governance.json',{evidenceHash,records:govRecords});
const council=decoded('Council.Members',(await rows('Council.Members'))[0][1]).map(aid).map(a=>a===old?newOwner:a).sort();save('council.json',council);
copy('staking-history.json',join(capture,'../checkpoint-staking-history-diagnostic-review.json'));
copy('staking-roles.json',join(capture,'../fixtures/staking-role-review-v1/review.json'));
copy('operators.scale',join(capture,'../fixtures/bootstrap-binding-v2/operators.scale'));
copy('democracy.json',join(capture,'../checkpoint-democracy-diagnostic-v2-review.json'));
copy('scheduler.json',join(capture,'../scheduler-review-v1/review.json'));
copy('offences.json',join(capture,'../offences-review-v1/review.json'));
copy('elections.json',join(capture,'../fixtures/elections-review-v1/review.json'));
copy('bridge-history.json',join(capture,'../fixtures/checkpoint-bridge-history-diagnostic-v2-review.json'));
copy('bridge-history-evidence.json',join(capture,'../fixtures/checkpoint-bridge-history-diagnostic-v2-evidence.json'));
save('NEVER-DEPLOY.txt','PUBLIC FIXTURE ONLY. The replacement key is publicly derivable. No actual victim authorized this mapping. Never deploy, publish as a recovery package, or use these keys for any network.\n');
const outer={schema:'sora-nexus-public-victim-fixture-inputs-v1',fixtureOnly:true,productionReady:false,checkpoint,old,newOwner,evidenceHash,capture,files:outputs,diagnosticOnlyReviews:['democracy.json'],capturedPages:[...seenPages].map(([file,sha256])=>({file,sha256})),outstanding:['Exact Rust capture/metadata verification and complete mapping-specific inventory','Native BridgeProxy pair transformation and terminal tombstone execution verification','Full composed guarded-engine and exact-artifact execution','All production approval/release gates listed in fixture-evidence.json']};
const outerBytes=JSON.stringify(outer,null,2)+'\n';writeFileSync(join(output,'FIXTURE-PROVENANCE.json'),outerBytes,{flag:'wx'});const outerCommitment=blake2AsHex(Buffer.from(outerBytes));
writeFileSync(join(output,'COMPOSE-ARGS.json'),JSON.stringify({command:'compose-storage-work',capture,csv:join(output,'mapping.csv'),proofs:join(output,'ownership-proofs.json'),inventory:join(output,'inventory.json'),economic:join(output,'economic.json'),semantic:join(output,'semantic.json'),governance:join(output,'governance.json'),referral:join(output,'referral.json'),'staking-controls':join(output,'staking-controls.json'),'staking-roles':join(output,'staking-roles.json'),'operators-scale':join(output,'operators.scale'),'staking-history':join(output,'staking-history.json'),scheduler:join(output,'scheduler.json'),offences:join(output,'offences.json'),elections:join(output,'elections.json'),'bridge-history':join(output,'bridge-history.json'),council:join(output,'council.json'),'evidence-commitment':outerCommitment,fixtureOnly:true,productionReady:false},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output,old,newOwner,files:outputs.length,evidenceCommitment:outerCommitment,fixtureOnly:true,productionReady:false,remaining:outer.outstanding}));
