#!/usr/bin/env node
/** Deterministic review inputs from public checkpoint bytes and a trusted CSV witness.
 * This prepares data for native compilation. It never signs, executes a historical
 * call, approves a refund, or claims that the resulting composition has passed. */
import {createReadStream} from 'node:fs';
import {readFile, writeFile, mkdir, lstat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {createRequire} from 'node:module';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {selectedRows} from './prepare_scheduler_review.mjs';
import {lines,parseTyped} from './independent_attacker_inventory.mjs';
import {sourceBase,catalogCapture,inventoryFromCatalog,streamDigest} from './checkpoint_inventory_catalog.mjs';
import {CHECKPOINT,MAX_PAGE_BYTES} from './capture_checkpoint.mjs';

export const DEFAULTS = Object.freeze({authorizationPolicy:'trusted-csv-v1',
  stakingPayee:'replacement-owner', retainedNominations:[], grants:'suspend-and-reconfirm',
  changedAuthorityPending:'cancel-with-liability-preservation', consensusKeys:'retain-checkpoint-unions',
  refunds:'none', bridgeExternalEvidence:'required-separately', productionReady:false});
export const ROLES = ['Staking.Nominators','Staking.Validators','Staking.CounterForNominators',
  'Staking.CounterForValidators','BagsList.ListNodes','BagsList.ListBags',
  'BagsList.CounterForListNodes','BagsList.__STORAGE_VERSION__'];
export const ELECTIONS = ['ElectionsPhragmen.Members','ElectionsPhragmen.RunnersUp',
  'ElectionsPhragmen.Candidates','ElectionsPhragmen.ElectionRounds','ElectionsPhragmen.Voting'];
export const GOVERNANCE = ['Council','TechnicalCommittee'].flatMap(p=>
  ['Members','Prime','Proposals','ProposalCount','ProposalOf','Voting','CostOf'].map(i=>`${p}.${i}`))
  .concat(['TechnicalMembership.Members','TechnicalMembership.Prime']);
export const HISTORY = ['ErasStakers','ErasStakersClipped','ErasStakersOverview','ErasStakersPaged',
  'ErasValidatorPrefs','ErasRewardPoints','ClaimedRewards','ValidatorSlashInEra','NominatorSlashInEra',
  'SlashingSpans','SpanSlash','UnappliedSlashes','Invulnerables','ErasTotalStake','ErasValidatorReward',
  'ErasStartSessionIndex','BondedEras'].map(i=>`Staking.${i}`);
export const ASYNC_ABSENT = ['AsyncUnappliedSlashes','OffenceQueue','OffenceQueueEras','ProcessingOffence',
  'HistoricalSlashes','HistoricalSlashHead','HistoricalSlashTail','LegacySlashCursor','CutoffEra',
  'FirstExecutionEra','ErasValidatorIncentiveWeight','ErasSumValidatorIncentiveWeight',
  'ErasValidatorIncentiveBudget','ErasSumWeightedPoints','ErasNominatorsSlashable','LastValidatorEra',
  'WeightedPointsFormulaStartEra','NextDeferredSlashEra','CancelledSlashes','VoterSnapshotStatus',
  'NextElectionPage','ElectableStashes','EraPruningState'].map(i=>`Staking.${i}`);
export const OFFENCES = ['Offences.Reports','Offences.ConcurrentReportsIndex'];
// This raw migration key is outside the metadata storage descriptors. Native
// staking-history admission includes it in the exact reviewed record set.
export const STAKING_HISTORY_MARKER = Object.freeze({
  storage:'StakingRewardPointsStashRemapped',
  key:'0x'+Buffer.from('runtime:migrations:staking_reward_points_stash_remapped').toString('hex'),
});
const CONTROL = ['Staking.Bonded','Staking.Ledger','Staking.Payee','Referrals.Referrers',
  'Referrals.Referrals','Democracy.VotingOf','Democracy.ReferendumInfoOf'];
const OPTIONAL_CONTROL = ['Proxy.Proxies','Proxy.Announcements'];
// Narrow source-reviewed scalar/key ownership moves. Collections containing
// accounts, consensus and pending authorizations remain native-adapter territory.
export const MUTABLE = Object.freeze({
  'System.Account':'Account key follows replacement with nonce, providers, consumers, sufficients and balance data unchanged.',
  'Tokens.Accounts':'Currency/account ownership follows replacement; preserve free, reserved and frozen quantities.',
  'Balances.Locks':'Account-owned lock IDs, amounts and withdrawal restrictions remain unchanged.',
  'Balances.Holds':'Account-owned hold reasons and quantities remain unchanged.',
  'Balances.Freezes':'Account-owned freeze IDs and quantities remain unchanged.',
  'Balances.Reserves':'Account-owned named reserve identifiers and quantities remain unchanged.',
  'PoolXYK.AccountPools':'Account-owned set of asset IDs moves unchanged; pool shares and reserves are separately conserved.',
  'Farming.PoolFarmers':'Unordered farmer ownership follows replacement in place; pool and lazy reward quantities remain unchanged.',
  'VestedRewards.Rewards':'Account-owned reward reasons, vested limit and total available amounts remain unchanged.',
  'ImOnline.AuthoredBlocks':'Session/validator counter follows replacement without changing the session index or authored count.',
  'Assets.AssetOwners':'Asset-ID to scalar owner; preserve asset and issuance records.',
  'CeresLiquidityLocker.LockerData':'Account key owns unchanged asset/amount/time lock vector.',
  'CeresTokenLocker.TokenLockerData':'Account key owns unchanged asset/amount/time lock vector.',
  'IrohaMigration.MigratedAccounts':'Completed external migration replay marker and effective referral recipient; presence preserved.',
  'Polkamarkt.ConditionCreators':'Scalar condition owner; preserve condition and resolution state.',
  'Polkamarkt.Markets':'Creator fee ownership follows replacement; preserve pool, status, outcome and financial fields.',
  'Polkamarkt.MarketPositions':'Account-keyed shares and claims follow replacement; market totals and collateral unchanged.',
  'Polkamarkt.DpmCostBasisByAccount':'Account-keyed cost basis follows replacement without payout or collateral change.',
  'Polkamarkt.MigratedLegacyPayouts':'Completed payout marker follows its effective owner; presence and amount unchanged.',
  'Polkamarkt.EarlyResolutionReports':'Reporter bond owner follows replacement; outcome, evidence and governance-only decision remain unchanged.',
});
const hex=b=>'0x'+Buffer.from(b).toString('hex');
const sha=b=>createHash('sha256').update(b).digest('hex');
const need=(value,message)=>{if(!value)throw Error(message);};
export function raw(value,size) {
  need(typeof value==='string'&&/^0x(?:[0-9a-f]{2})*$/.test(value),'canonical hexadecimal required');
  const bytes=Buffer.from(value.slice(2),'hex');need(size===undefined||bytes.length===size,'encoded width differs');return bytes;
}
const ordered=rows=>[...rows].sort((a,b)=>a.key.localeCompare(b.key));
const record=row=>({storage:row.storage,key:row.key,valueHash:row.valueHash});
const jsonBytes=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
export function compact(data,offset=0) {
  need(offset<data.length,'missing SCALE compact');const mode=data[offset]&3;
  const width=mode<3?1<<mode:(data[offset]>>>2)+5;
  need(width<=5&&offset+width<=data.length,'SCALE compact bounds');
  let n=0n;for(let i=width-1;i>=(mode===3?1:0);i--)n=n*256n+BigInt(data[offset+i]);
  if(mode<3)n>>=2n;
  need(mode===0||n>=[0n,64n,16384n,1073741824n][mode],'noncanonical SCALE compact');
  need(n<=0xffffffffn,'SCALE count bound');return [Number(n),width];
}
export function mappingRows(witness) {
  need(witness.schema==='sora-nexus-trusted-csv-mapping-v1'&&witness.authorizationPolicy==='trusted-csv-v1'
    &&witness.ownershipSignaturesRequired===false&&witness.productionReady===false
    &&witness.unusedDestinationsVerified===true&&witness.allEntriesClassified===true,'trusted mapping witness scope');
  const bytes=raw(witness.replacementsScale),[count,offset]=compact(bytes);
  need(count>0&&count<=4096&&bytes.length===offset+count*64&&witness.expandedMappings===count,'expanded mapping bounds');
  const rows=Array.from({length:count},(_,i)=>[hex(bytes.subarray(offset+i*64,offset+i*64+32)),hex(bytes.subarray(offset+i*64+32,offset+i*64+64))]);
  need(rows.every((row,i)=>row[0]!==row[1]&&(!i||rows[i-1][0]<row[0])),'mapping order or self-substitution');
  const old=new Set(rows.map(r=>r[0])),next=new Set(rows.map(r=>r[1]));
  need(next.size===count&&![...next].some(key=>old.has(key)),'mapping collisions');return rows;
}
export function directVoteIndices(value) {
  const bytes=raw(value);need(bytes.length>0,'missing voting value');
  if(bytes[0]===1)return []; // Native compiler validates the complete Delegating variant.
  need(bytes[0]===0,'unsupported voting variant');let [count,size]=compact(bytes,1),offset=1+size;
  need(count<=500,'vote count bound');const indices=[];
  for(let i=0;i<count;i++) {
    need(offset+5<=bytes.length,'truncated vote');indices.push(bytes.readUInt32LE(offset));offset+=4;
    const kind=bytes[offset++],width=[17,32,48][kind];need(width!==undefined&&offset+width<=bytes.length,'unsupported/truncated account vote');offset+=width;
  }
  need(bytes.length===offset+52,'direct voting aggregate/prior layout differs');
  need(indices.every((n,i)=>!i||indices[i-1]<n),'duplicate/unsorted referendum votes');return indices;
}
export function mergeDerived(existing,generated) {
  need(Array.isArray(existing)&&Array.isArray(generated),'derived control arrays required');const rows=new Map();
  for(const row of [...existing,...generated]) {
    raw(row.old,32);raw(row.new,32);
    need(!rows.has(row.old)||JSON.stringify(rows.get(row.old))===JSON.stringify(row),'derived control conflicts with native discovery');
    rows.set(row.old,row);
  }
  return [...rows.values()].sort((a,b)=>a.old.localeCompare(b.old));
}

/** Pure graph assembly. The caller supplies only canonical, hash-checked typed rows. */
export function generate({rows,descriptors,prefix,keyFor,decodeAccountVector,mapping,operatorsHash,evidenceHash,baseInventory,baseSemantic}) {
  const mapped=new Map(mapping),byStorage=new Map();
  for(const row of rows.values()) {if(!byStorage.has(row.storage))byStorage.set(row.storage,[]);byStorage.get(row.storage).push(row);}
  const get=name=>ordered(byStorage.get(name)??[]);
  const required=(name,key,nullable=false)=>{const row=rows.get(key);need(row?.storage===name||nullable&&!row,`missing backing row ${name} ${key}`);return row?record(row):{storage:name,key,valueHash:null};};
  const complete=(names,absence)=>names.flatMap(name=>{const values=get(name);if(values.length)return values.map(record);
    return absence==='all'||absence==='plain'&&descriptors.get(name)?.hashers.length===0?[{storage:name,key:prefix(name),valueHash:null}]:[];});
  const stakingMarker=rows.get(STAKING_HISTORY_MARKER.key);
  need(stakingMarker?.storage===STAKING_HISTORY_MARKER.storage&&stakingMarker.value==='0x01',
    'staking reward controller-to-stash normalization marker must be present and complete');
  raw(stakingMarker.valueHash,32);
  const reviews={
    governance:{evidenceHash,records:complete(GOVERNANCE,'plain')},
    elections:{evidenceHash,records:complete(ELECTIONS,'all')},
    'staking-roles':{evidenceHash,operatorsHash,records:complete(ROLES),retainedNominations:[]},
    'staking-history':{evidenceHash,records:[...complete(HISTORY),record(stakingMarker)],absentPrefixes:[...HISTORY.filter(name=>!get(name).length),...ASYNC_ABSENT].sort()},
    offences:{evidenceHash,records:complete(OFFENCES)},
  };
  need(ASYNC_ABSENT.every(name=>!get(name).length),'async state unexpectedly present at checkpoint');
  const controls=[],pairs=[],blockers=[];
  for(const bond of get('Staking.Bonded')) {
    const controller=hex(raw(bond.value,32));
    if(mapped.has(controller)&&!mapped.has(bond.owner))blockers.push({storage:bond.storage,key:bond.key,
      reason:'Mapped controller controls an unmapped stash; existing native adapter requires an explicit owner repair.'});
    if(mapped.has(bond.owner)) {
      const ledger=rows.get(keyFor('Staking.Ledger',controller));
      need(ledger&&hex(raw(ledger.value).subarray(0,32))===bond.owner,'staking Bonded/Ledger relation differs');
      pairs.push({stash:bond.owner,controller,bondedHash:bond.valueHash,ledgerHash:ledger.valueHash});
    }
  }
  if(pairs.length)reviews['staking-controls']={evidenceHash,pairs:pairs.sort((a,b)=>a.stash.localeCompare(b.stash))};
  for(const row of get('Staking.Payee')) if(mapped.has(row.owner)) controls.push({storage:row.storage,key:row.key,
    expectedValueHash:row.valueHash,action:'default_staking_payee',reviewEvidenceHash:evidenceHash});
  for(const name of ['Proxy.Proxies','Proxy.Announcements'])for(const row of get(name))
    if(mapped.has(row.owner)||(row.references??[]).some(r=>mapped.has(r.identity)))
      blockers.push({storage:name,key:row.key,reason:'Existing proxy adapter requires native deposit custody/reclaim accounting; no automatic deletion or refund.'});
  const referralRows=new Map(),affected=new Set();
  for(const row of get('Referrals.Referrers')) {
    const referrer=hex(raw(row.value,32));if(mapped.has(row.owner)||mapped.has(referrer)) {
      referralRows.set(row.key,record(row));affected.add(row.owner);affected.add(referrer);
    }
  }
  for(const row of get('Referrals.Referrals'))if(mapped.has(row.owner)||decodeAccountVector(row.value).some(owner=>mapped.has(owner)))referralRows.set(row.key,record(row));
  for(const owner of affected) {const row=required('System.Account',keyFor('System.Account',owner));referralRows.set(row.key,row);}
  if(referralRows.size)reviews.referral={evidenceHash,records:ordered(referralRows.values())};
  const votes=get('Democracy.VotingOf');
  if(votes.length) {
    const records=complete(['Democracy.VotingOf','Democracy.ReferendumInfoOf']),referenda=new Set(get('Democracy.ReferendumInfoOf').map(row=>row.key));
    const absent=new Set(votes.flatMap(row=>directVoteIndices(row.value)).map(index=>keyFor('Democracy.ReferendumInfoOf',index)).filter(key=>!referenda.has(key)));
    for(const key of [...absent].sort())records.push({storage:'Democracy.ReferendumInfoOf',key,valueHash:null});
    for(const row of votes)if(mapped.has(row.owner))for(const name of ['System.Account','Balances.Locks'])records.push(required(name,keyFor(name,row.owner),true));
    reviews.democracy={evidenceHash,records:ordered(records)};
  }
  const inventory=structuredClone(baseInventory),semantic=structuredClone(baseSemantic);
  for(const row of inventory.dispositions??[])if(row.trie==='top'&&Object.hasOwn(MUTABLE,row.storage)) {
    row.action='mutable_ownership';row.reason=MUTABLE[row.storage];row.evidenceHash=evidenceHash;
  }
  const existing=new Set((semantic.controls??[]).map(row=>row.key));
  for(const row of controls)need(!existing.has(row.key),'generated control conflicts with base semantic row');
  semantic.controls=[...(semantic.controls??[]),...controls].sort((a,b)=>a.key.localeCompare(b.key));
  return {reviews,inventory,semantic,blockers};
}

async function bounded(path,maximum=64*1024**2) {
  const info=await lstat(path);need(info.isFile()&&!info.isSymbolicLink()&&info.size<=maximum,`unaliased bounded input file required: ${path} (${info.size} bytes; maximum ${maximum})`);
  const data=await readFile(path);need(data.length===info.size,`input changed while reading: ${path}`);return data;
}
export async function main(args) {
  const options={};for(let i=0;i<args.length;i+=2){need(args[i]?.startsWith('--')&&args[i+1],'expected --name value');const key=args[i].slice(2);need(!Object.hasOwn(options,key),'duplicate option');options[key]=resolve(args[i+1]);}
  const required=['capture','verification','metadata-witness','trusted-mapping','csv','operators','base-inventory','base-semantic','output'];
  need(Object.keys(options).length===required.length&&required.every(key=>options[key]),'required paths: '+required.join(', '));
  const workspace=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),capture=options.capture;
  const inputs=new Map(),load=async(path,limit)=>{const data=await bounded(path,limit);inputs.set(path,sha(data));return data;};
  const capturePages=new Map(),checkPage=async(page)=>{const path=join(capture,page.file),digest=await streamDigest(path,MAX_PAGE_BYTES);need(digest===page.sha256,`capture page checksum differs: ${path}`);capturePages.set(path,digest);return path;};
  const manifestRaw=await load(join(capture,'capture-complete.json')),manifest=JSON.parse(manifestRaw);
  const verified=JSON.parse(await load(options.verification)),witness=JSON.parse(await load(options['metadata-witness']));
  const trustedRaw=await load(options['trusted-mapping']),trusted=JSON.parse(trustedRaw),mapping=mappingRows(trusted);
  const inventoryRaw=await load(options['base-inventory']),baseInventory=JSON.parse(inventoryRaw);
  const semanticRaw=await load(options['base-semantic']),baseSemantic=JSON.parse(semanticRaw);
  const operators=await load(options.operators,1024**2);
  const csv=await load(options.csv,2*1024**2);
  need(operators.subarray(0,8).equals(Buffer.from('snxops02')),'checkpoint operator v2 required');
  need(manifest.complete===true&&manifest.checkpoint.hash===CHECKPOINT.hash&&manifest.checkpoint.stateRoot===CHECKPOINT.stateRoot
    &&manifest.children.length===0,'complete pinned checkpoint with no child tries required');
  need(verified.stateRootVerified===true&&verified.headerHashVerified===true&&verified.checkpointHash===CHECKPOINT.hash
    &&verified.stateRoot===CHECKPOINT.stateRoot&&verified.captureManifestSha256===sha(manifestRaw),'root-verified capture report differs');
  need(trusted.checkpointHash===CHECKPOINT.hash&&trusted.checkpointStateRoot===CHECKPOINT.stateRoot
    &&trusted.captureManifestSha256===sha(manifestRaw)&&trusted.inventorySha256===sha(inventoryRaw)
    &&trusted.semanticPlanSha256===sha(semanticRaw)&&trusted.mappingCsvSha256===sha(csv),'trusted report input joins differ');
  const metadataBytes=await load(join(capture,'runtime-metadata.scale'));
  need(witness.wasmExecutionVerified===true&&witness.nativeExecution===false&&witness.checkpointHash===CHECKPOINT.hash
    &&witness.stateRoot===CHECKPOINT.stateRoot&&witness.metadataSha256===sha(metadataBytes)
    &&manifest.artifacts['runtime-metadata.scale'].sha256===sha(metadataBytes),'executing metadata witness differs');
  const require=createRequire(join(workspace,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
  const {Metadata,TypeRegistry}=require('@polkadot/types'),{blake2AsU8a,xxhashAsU8a}=require('@polkadot/util-crypto');
  const blake=bytes=>hex(blake2AsU8a(bytes,256));
  need(blake(raw(trusted.replacementsScale))===trusted.expandedMappingCommitment,'expanded mapping commitment differs');
  need(trusted.metadataHash===blake(metadataBytes),'trusted mapping metadata identity differs');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);
  const portable=metadata.asLatest.toJSON(),types=new Map(portable.lookup.types.map(t=>[t.id,t.type])),descriptors=new Map();
  const prefix=name=>{const [pallet,item]=name.split('.');return hex(Buffer.concat([xxhashAsU8a(pallet,128),xxhashAsU8a(item==='__STORAGE_VERSION__'?':__STORAGE_VERSION__:':item,128)]));};
  for(const pallet of portable.pallets)if(pallet.storage)for(const item of pallet.storage.items){
    const map=item.type.map,name=`${pallet.name}.${item.name}`;
    const keys=map?(map.hashers.length===1?[map.key]:types.get(map.key)?.def.tuple):[];
    need(!map||keys?.length===map.hashers.length,'metadata key tuple differs');
    need(prefix(name)===hex(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(item.name,128)])),'native pallet prefix differs');
    descriptors.set(name,{name,prefix:prefix(name),hashers:map?.hashers??[],keyTypes:keys,valueType:map?.value??item.type.plain});
  }
  const u16=[...types].find(([,type])=>type.def.primitive==='U16')?.[0];need(u16!==undefined,'storage version type missing');
  for(const pallet of portable.pallets) {
    const name=`${pallet.name}.__STORAGE_VERSION__`;need(!descriptors.has(name),'storage-version descriptor collision');
    descriptors.set(name,{name,prefix:prefix(name),hashers:[],keyTypes:[],valueType:u16});
  }
  const legacyRaw=await load(join(workspace,'misc/incident_recovery/checkpoint_legacy_schema.json'));
  const legacy=JSON.parse(legacyRaw);
  need(legacy.checkpointHash===CHECKPOINT.hash&&legacy.metadataBlake2===blake(metadataBytes)
    &&legacy.metadataSha256===sha(metadataBytes)&&legacy.originalTypeCount===types.size,'source-pinned legacy metadata differs');
  const catalogDescriptors=new Map([...descriptors.values()].map(row=>[row.prefix,row]));
  need(catalogDescriptors.size===descriptors.size,'metadata storage prefix collision');
  for(const row of legacy.storage){need(row.trie==='top'&&!catalogDescriptors.has(row.prefix),'legacy prefix collision');catalogDescriptors.set(row.prefix,{...row,name:row.name});}
  catalogDescriptors.set('0x3a636f6465',{prefix:'0x3a636f6465',name:'unknown:0x3a636f6465'});
  const sourceInventory=sourceBase(baseInventory,legacy,prefix('Preimage.PreimageFor'));
  const observedPages=[];
  const catalog=await catalogCapture(manifest,catalogDescriptors,async function*(page) {
    const path=await checkPage(page);
    yield* lines(createReadStream(path).pipe(createGunzip()));
  },page=>observedPages.push(page));
  const names=[...new Set([...GOVERNANCE,...ROLES,...ELECTIONS,...HISTORY,...OFFENCES,...CONTROL,
    ...OPTIONAL_CONTROL.filter(name=>descriptors.has(name))])];
  for(const name of names)need(descriptors.has(name),`missing pinned descriptor ${name}`);
  const byPrefix=new Map([...descriptors.values()].map(d=>[d.prefix,d])),rows=new Map(),pages=new Map();
  const keyFor=(name,identity)=>{const d=descriptors.get(name);need(d&&d.hashers.length===1,'single-key native storage required');
    const value=typeof identity==='number'?Buffer.alloc(4):raw(identity,32);if(typeof identity==='number')value.writeUInt32LE(identity);
    const hasher=d.hashers[0],hash=hasher==='Twox64Concat'?xxhashAsU8a(value,64):hasher==='Blake2_128Concat'?blake2AsU8a(value,128):null;
    need(hash,'unsupported account/index hasher');return d.prefix+hex(Buffer.concat([hash,value])).slice(2);};
  async function scan(prefixes) {
    for(const page of observedPages) {
      const selected=prefixes.filter(p=>page.keyCount&&page.firstKey<=p+'f'.repeat(1024)&&page.lastKey>=p);if(!selected.length)continue;
      need(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'capture page path differs');
      const path=await checkPage(page);pages.set(page.file,page.sha256);
      for await(const [key,value]of selectedRows(createReadStream(path).pipe(createGunzip()),selected)) {
        const d=byPrefix.get(key.slice(0,66));need(d,`unexpected selected storage ${key}`);
        const bytes=raw(value),parsed=parseTyped(bytes,d.valueType,types,{render:false,watched:{has:()=>true}});
        need(parsed.consumed===bytes.length&&Buffer.from(registry.createTypeUnsafe(`Lookup${d.valueType}`,[bytes]).toU8a()).equals(bytes),'typed value roundtrip differs');
        let owner;
        if(d.hashers.length===1&&['Staking.Bonded','Staking.Ledger','Staking.Payee','Referrals.Referrers','Referrals.Referrals','Democracy.VotingOf','Proxy.Proxies','Proxy.Announcements','System.Account','Balances.Locks'].includes(d.name)) {
          owner=hex(raw(key).subarray(-32));need(keyFor(d.name,owner)===key,'native account key hash differs');
        }
        const row={storage:d.name,key,value,valueHash:blake(bytes),references:parsed.references,...(owner?{owner}:{})};
        need(!rows.has(key)||JSON.stringify(rows.get(key))===JSON.stringify(row),'conflicting duplicate selected row');rows.set(key,row);
        need(rows.size<=200000,'review graph row bound');
      }
    }
  }
  await scan(names.map(prefix));
  // Select the exact raw key from authenticated pages; do not synthesize the
  // marker from runtime source or from a previous review's expected value.
  for(const page of observedPages) {
    if(!page.keyCount||page.firstKey>STAKING_HISTORY_MARKER.key||page.lastKey<STAKING_HISTORY_MARKER.key)continue;
    const path=await checkPage(page);pages.set(page.file,page.sha256);
    for await(const [key,value]of selectedRows(createReadStream(path).pipe(createGunzip()),[STAKING_HISTORY_MARKER.key])) {
      if(key!==STAKING_HISTORY_MARKER.key)continue;
      need(!rows.has(key),'duplicate staking history migration marker');
      rows.set(key,{storage:STAKING_HISTORY_MARKER.storage,key,value,valueHash:blake(raw(value)),references:[]});
    }
  }
  // Reject async prefixes by reading them even though deployed metadata has no descriptors.
  const absentOptional=OPTIONAL_CONTROL.filter(name=>!descriptors.has(name));
  const asyncPrefixes=new Map([...ASYNC_ABSENT,...absentOptional].map(name=>[prefix(name),name]));
  for(const page of observedPages) {
    const selected=[...asyncPrefixes.keys()].filter(p=>page.keyCount&&page.firstKey<=p+'f'.repeat(1024)&&page.lastKey>=p);
    if(selected.length) {
      const path=await checkPage(page);pages.set(page.file,page.sha256);
      for await(const row of selectedRows(createReadStream(path).pipe(createGunzip()),selected))throw Error(`unexpected ${asyncPrefixes.get(row[0].slice(0,66))} row ${row[0]}`);
    }
  }
  const mapped=new Set(mapping.map(row=>row[0])),backing=new Set();
  for(const row of rows.values()) {
    if(row.storage==='Referrals.Referrers') {const other=hex(raw(row.value,32));if(mapped.has(row.owner)||mapped.has(other))for(const owner of [row.owner,other])backing.add(keyFor('System.Account',owner));}
    if(row.storage==='Democracy.VotingOf'&&mapped.has(row.owner))for(const name of ['System.Account','Balances.Locks'])backing.add(keyFor(name,row.owner));
  }
  await scan([...backing]);
  const sources=[];for(const name of ['Cargo.lock','runtime/src/lib.rs','misc/incident_recovery/prepare_checkpoint_reviews.mjs','misc/incident_recovery/prepare_scheduler_review.mjs',
    'misc/incident_recovery/checkpoint_inventory_catalog.mjs','misc/incident_recovery/checkpoint_legacy_schema.json',
    'misc/incident_recovery/independent_attacker_inventory.mjs',...['inventory','governance','referral','staking_controls','staking_roles','staking_history','democracy','elections','offences','substitution_semantics','session_controls'].map(n=>`utils/incident-recovery/src/${n}.rs`),
    ...['assets','ceres-liquidity-locker','ceres-token-locker','iroha-migration','polkamarkt','pool-xyk','farming','vested-rewards'].map(p=>`pallets/${p}/src/lib.rs`)])sources.push({file:name,sha256:sha(await load(join(workspace,name)))});
  const evidence={schema:'sora-nexus-automatic-checkpoint-review-evidence-v1',checkpointHash:CHECKPOINT.hash,stateRoot:CHECKPOINT.stateRoot,
    authorizationPolicy:'trusted-csv-v1',defaults:DEFAULTS,trustedCsvCommitment:trusted.trustedCsvCommitment,
    expandedMappingCommitment:trusted.expandedMappingCommitment,operatorsHash:blake(operators),
    inputSha256:{...Object.fromEntries(required.filter(name=>!['capture','output'].includes(name)).map(name=>[name,inputs.get(options[name])])),
      checkpointManifest:sha(manifestRaw),checkpointMetadata:sha(metadataBytes)},
    sources,pages:Object.fromEntries([...pages].sort(([a],[b])=>a.localeCompare(b))),selectedRows:rows.size,absentOptionalControlPrefixes:absentOptional,
    selectedRecordsSha256:sha(jsonBytes(ordered(rows.values()).map(record))),
    completePrefixCatalogSha256:sha(jsonBytes(catalog)),catalogEntries:catalog.reduce((sum,row)=>sum+row.entries,0),
    observedPageRangesSha256:sha(jsonBytes(observedPages)),
    catalogPages:manifest.top.pages.length,pageRangeHintsTrusted:false,
    humanApprovalClaimed:false,ownershipSignaturesClaimed:false,nativeCompositionVerified:false,productionReady:false};
  const evidenceBytes=jsonBytes(evidence),evidenceHash=blake(evidenceBytes);
  const decodeAccountVector=value=>{const bytes=raw(value),[count,offset]=compact(bytes);need(count<=4096&&bytes.length===offset+count*32,'referral vector layout differs');return Array.from({length:count},(_,i)=>hex(bytes.subarray(offset+i*32,offset+(i+1)*32)));};
  const result=generate({rows,descriptors,prefix,keyFor,decodeAccountVector,mapping,operatorsHash:blake(operators),evidenceHash,baseInventory,baseSemantic});
  result.inventory=inventoryFromCatalog(sourceInventory,catalog,MUTABLE,evidenceHash);
  // Derived utility controls are produced by native checkpoint discovery, not invented here.
  result.semantic.derivedAccounts=mergeDerived(baseSemantic.derivedAccounts??[],trusted.derivedAccounts);
  for(const [path,digest]of inputs)need(sha(await bounded(path,128*1024**2))===digest,'input/source changed during review preparation');
  for(const [path,digest]of capturePages)need(await streamDigest(path,MAX_PAGE_BYTES)===digest,`capture page changed during review preparation: ${path}`);
  await mkdir(options.output,{recursive:false});
  const outputs={};const save=async(name,value)=>{const bytes=Buffer.isBuffer(value)?value:jsonBytes(value);await writeFile(join(options.output,name),bytes,{flag:'wx'});outputs[name]={sha256:sha(bytes),blake2_256:blake(bytes),bytes:bytes.length};};
  await save('evidence.json',evidenceBytes);await save('prefix-catalog.json',catalog);await save('observed-page-ranges.json',observedPages);await save('inventory.json',result.inventory);await save('semantic.json',result.semantic);
  for(const [name,review]of Object.entries(result.reviews))await save(`${name}.json`,review);
  await save('generation.json',{schema:'sora-nexus-automatic-checkpoint-reviews-v1',outputs,blockers:result.blockers,
    composeFlags:Object.keys(result.reviews),authorizationPolicy:'trusted-csv-v1',humanApprovalClaimed:false,
    nativeCompositionVerified:false,productionReady:false,
    remaining:'Native recomposition, Session adapter, bridge/authentication evidence, economic coverage, history/finality and exact-artifact rehearsal gates remain required.'});
  process.stdout.write(JSON.stringify({output:options.output,reviews:Object.keys(result.reviews),blockers:result.blockers.length,nativeCompositionVerified:false,productionReady:false})+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main(process.argv.slice(2)).catch(error=>{console.error(error.message);process.exitCode=1;});
