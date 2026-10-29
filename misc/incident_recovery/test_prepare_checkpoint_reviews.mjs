// Synthetic graph assembly tests. No runtime, real capture or approval evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {generate,mappingRows,directVoteIndices,mergeDerived,MUTABLE,GOVERNANCE,ELECTIONS,ROLES,HISTORY,ASYNC_ABSENT,OFFENCES} from './prepare_checkpoint_reviews.mjs';
const hx=n=>'0x'+n.toString(16).padStart(2,'0').repeat(32);
const hash=bytes=>'0x'+createHash('sha256').update(bytes).digest('hex'); // Synthetic row labels, never native evidence.
const prefix=name=>hash(Buffer.from(name));
const keyFor=(name,owner)=>prefix(name)+(typeof owner==='number'?owner.toString(16).padStart(8,'0'):owner.slice(2));
function voting(indices=[]) {
  const rows=indices.map(index=>{const bytes=Buffer.alloc(22);bytes.writeUInt32LE(index);return bytes;});
  return '0x'+Buffer.concat([Buffer.from([0,indices.length<<2]),...rows,Buffer.alloc(52)]).toString('hex');
}
function fixture() {
  const rows=new Map(),descriptors=new Map();
  for(const name of [...GOVERNANCE,...ELECTIONS,...ROLES,...HISTORY,...OFFENCES])descriptors.set(name,{hashers:[]});
  for(const name of GOVERNANCE.filter(name=>/ProposalOf|Voting|CostOf/.test(name)))descriptors.get(name).hashers=['Identity'];
  const add=(name,owner,value='0x00',extra={})=>{
    const key=owner===null?prefix(name):keyFor(name,owner);
    const row={storage:name,key,owner,value,valueHash:hash(Buffer.from(value)),...extra};rows.set(key,row);return row;
  };
  const options={rows,descriptors,prefix,keyFor,decodeAccountVector:value=>JSON.parse(value),mapping:[[hx(1),hx(2)]],
    operatorsHash:hx(90),evidenceHash:hx(91),baseInventory:{dispositions:[
      {trie:'top',storage:'Assets.AssetOwners',action:'retain_historical',reason:'old',evidenceHash:hx(9)},
      {trie:'top',storage:'Session.Validators',action:'retain_historical',reason:'native-only',evidenceHash:hx(9)},
      {trie:'top',storage:'IrohaMigration.PendingMultiSigAccounts',action:'retain_historical',reason:'unresolved',evidenceHash:hx(9)}]},
    baseSemantic:{bridgeCancellations:[{retainedSyntheticOpaqueEvidence:true}],derivedAccounts:[]}};
  return {rows,add,options};
}
test('review family constants stay aligned with native complete inventories',async()=>{
  for(const [file,constant,expected,prefixName]of [
    ['staking_roles','ITEMS',ROLES,''],['staking_history','ITEMS',HISTORY,'Staking.'],
    ['staking_history','ASYNC_ABSENT',ASYNC_ABSENT,'Staking.'],['elections','ITEMS',ELECTIONS,'']]) {
    const source=await readFile(new URL(`../../utils/incident-recovery/src/${file}.rs`,import.meta.url),'utf8');
    const body=source.match(new RegExp(`(?:pub )?const ${constant}: &[\\s\\S]*?= &\\[([\\s\\S]*?)\\];`))?.[1];
    assert.ok(body,`${file} ${constant}`);
    const names=[...body.matchAll(/"([A-Za-z_.]+)"/g)].map(match=>prefixName+match[1]);assert.deepEqual(expected,names);
  }
});
test('only narrow scalar/key ownership families change disposition; native families retain admission',()=>{
  const f=fixture(),before=structuredClone(f.options.baseInventory),result=generate(f.options);
  assert.equal(result.inventory.dispositions[0].action,'mutable_ownership');
  assert.deepEqual(result.inventory.dispositions.slice(1),before.dispositions.slice(1));
  assert.deepEqual(f.options.baseInventory,before);assert.deepEqual(result.semantic.bridgeCancellations,f.options.baseSemantic.bridgeCancellations);
  assert.equal(Object.hasOwn(MUTABLE,'Permissions.Owners'),false);assert.equal(Object.hasOwn(MUTABLE,'Session.NextKeys'),false);
  assert.equal(result.reviews['staking-roles'].retainedNominations.length,0);
});
test('plain governance absence and empty election-map absence are explicit but empty governance maps omitted',()=>{
  const f=fixture(),result=generate(f.options);
  assert.ok(result.reviews.governance.records.some(row=>row.storage==='Council.Members'&&row.valueHash===null));
  assert.equal(result.reviews.governance.records.some(row=>row.storage==='Council.Voting'),false);
  assert.ok(result.reviews.elections.records.some(row=>row.storage==='ElectionsPhragmen.Voting'&&row.valueHash===null));
  assert.deepEqual(result.reviews['staking-history'].absentPrefixes,[...HISTORY,...ASYNC_ABSENT].sort());
  f.add(ASYNC_ABSENT[0],hx(1));assert.throws(()=>generate(f.options),/async state/);
});
test('staking reviews bind paired financial ledger bytes and default only mapped stash payees',()=>{
  const f=fixture(),bond=f.add('Staking.Bonded',hx(1),hx(7)),ledger=f.add('Staking.Ledger',hx(7),hx(1)+'00');
  const payee=f.add('Staking.Payee',hx(1));f.add('Staking.Payee',hx(6));
  const result=generate(f.options);
  assert.deepEqual(result.reviews['staking-controls'].pairs,[{stash:hx(1),controller:hx(7),bondedHash:bond.valueHash,ledgerHash:ledger.valueHash}]);
  assert.deepEqual(result.semantic.controls,[{storage:'Staking.Payee',key:payee.key,expectedValueHash:payee.valueHash,action:'default_staking_payee',reviewEvidenceHash:hx(91)}]);
  f.rows.delete(ledger.key);assert.throws(()=>generate(f.options),/Bonded\/Ledger/);
});
test('referral review includes affected forward/reverse rows and both backing accounts only',()=>{
  const f=fixture(),forward=f.add('Referrals.Referrers',hx(1),hx(7)),reverse=f.add('Referrals.Referrals',hx(7),JSON.stringify([hx(1)]));
  const first=f.add('System.Account',hx(1)),second=f.add('System.Account',hx(7));
  f.add('Referrals.Referrers',hx(8),hx(9));f.add('Referrals.Referrals',hx(9),JSON.stringify([hx(8)]));
  const result=generate(f.options);
  assert.deepEqual(new Set(result.reviews.referral.records.map(row=>row.key)),new Set([forward,reverse,first,second].map(row=>row.key)));
  f.rows.delete(second.key);assert.throws(()=>generate(f.options),/backing row/);
});
test('Democracy records retain all votes and explicitly bind missing referendum/backing rows',()=>{
  const f=fixture(),vote=f.add('Democracy.VotingOf',hx(1),voting([17]));f.add('Democracy.VotingOf',hx(8),voting([]));
  const result=generate(f.options),records=result.reviews.democracy.records;
  assert.equal(records.filter(row=>row.storage==='Democracy.VotingOf').length,2);
  for(const [name,key]of [['Democracy.ReferendumInfoOf',keyFor('Democracy.ReferendumInfoOf',17)],['Balances.Locks',keyFor('Balances.Locks',hx(1))],['System.Account',keyFor('System.Account',hx(1))]])
    assert.ok(records.some(row=>row.storage===name&&row.key===key&&row.valueHash===null));
  assert.ok(records.some(row=>row.key===vote.key&&row.valueHash===vote.valueHash));
  assert.deepEqual(directVoteIndices(voting([1,7])),[1,7]);
  assert.throws(()=>directVoteIndices(voting([7,1])),/unsorted/);assert.throws(()=>directVoteIndices(voting([1])+'00'),/layout/);
});
test('unresolved proxy custody produces a blocker and no destructive control',()=>{
  const f=fixture();f.add('Proxy.Proxies',hx(1));f.add('Proxy.Announcements',hx(3),'0x00',{references:[{identity:hx(1)}]});
  const result=generate(f.options);assert.equal(result.blockers.length,2);assert.deepEqual(result.semantic.controls,[]);
});
test('trusted expansion retains explicit controls and rejects conflicting generated derivations',()=>{
  const prior={old:hx(3),new:hx(4),derivation:{kind:'multisig',signatories:[hx(1)],threshold:1}};
  const generated={old:hx(5),new:hx(6),derivation:{kind:'utility',controller:hx(1),index:7}};
  assert.deepEqual(mergeDerived([prior],[generated]),[prior,generated]);
  assert.deepEqual(mergeDerived([generated],[generated]),[generated]);
  assert.throws(()=>mergeDerived([generated],[{...generated,new:hx(7)}]),/conflict/);
});
test('mapping rows require explicit trusted-CSV scope, exact SCALE and injective disjoint identities',()=>{
  const witness={schema:'sora-nexus-trusted-csv-mapping-v1',authorizationPolicy:'trusted-csv-v1',ownershipSignaturesRequired:false,
    productionReady:false,unusedDestinationsVerified:true,allEntriesClassified:true,expandedMappings:1,replacementsScale:'0x04'+hx(1).slice(2)+hx(2).slice(2)};
  assert.deepEqual(mappingRows(witness),[[hx(1),hx(2)]]);
  for(const update of [{ownershipSignaturesRequired:true},{productionReady:true},{unusedDestinationsVerified:false},{expandedMappings:2},
    {replacementsScale:witness.replacementsScale+'00'},{replacementsScale:'0x04'+hx(1).slice(2).repeat(2)}])
    assert.throws(()=>mappingRows({...witness,...update}));
});
