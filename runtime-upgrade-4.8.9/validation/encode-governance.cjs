// Usage: NODE_PATH=<directory containing @polkadot/api> node validation/encode-governance.cjs
// Reads public chain metadata and writes unsigned calls. No signing or submission.
const { ApiPromise, WsProvider } = require('@polkadot/api');
const {TypeRegistry, Metadata}=require('@polkadot/types');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { u8aToHex } = require('@polkadot/util');
const { readFileSync, writeFileSync } = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root=path.resolve(__dirname,'..');
const timeout=setTimeout(()=>{console.error('Call encoding timed out');process.exit(2)},60000);
async function main(){
 const live=JSON.parse(readFileSync(path.join(__dirname,'live-chain.json')));
 const api=await ApiPromise.create({provider:new WsProvider(live.endpoint)});
 try {
  assert.equal(api.runtimeVersion.specVersion.toNumber(),130);
  assert.equal(api.genesisHash.toHex(),live.genesisHash);
  const pinnedRegistry=new TypeRegistry();
  pinnedRegistry.setMetadata(new Metadata(pinnedRegistry,readFileSync(path.join(__dirname,'live-metadata.scale'))));
  const at=api;
  const wasm=readFileSync(path.join(root,'framenode-runtime-4.8.9.compact.compressed.wasm'));
  const call=at.tx.system.setCode(u8aToHex(wasm)).method;
  const bytes=Buffer.from(call.toU8a());
  assert.deepEqual(bytes,readFileSync(path.join(root,'set-code-call.scale')));
  const hash=blake2AsHex(bytes,256),len=bytes.length;
  const note=at.tx.preimage.notePreimage(call.toHex()).method;
  const external=at.tx.democracy.externalProposeMajority({Lookup:{hash,len}}).method;
  const fast=at.tx.democracy.fastTrack(hash,live.fastTrackVotingPeriod,0).method;
  const councilCount=live.councilMembers.length,techCount=live.technicalCommitteeMembers.length;
  const minimum=Math.ceil(councilCount/2),conservative=Math.floor(councilCount/2)+1,techThreshold=Math.floor(techCount/2)+1;
  const calls={
   'set-code-call':call,
   'preimage-note-call':note,
   'democracy-external-propose-majority-call':external,
   [`council-propose-external-majority-threshold-${minimum}-call`]:at.tx.council.propose(minimum,external,external.encodedLength).method,
   [`council-propose-external-majority-threshold-${conservative}-call`]:at.tx.council.propose(conservative,external,external.encodedLength).method,
   'democracy-fast-track-call':fast,
   [`technical-committee-fast-track-threshold-${techThreshold}-call`]:at.tx.technicalCommittee.propose(techThreshold,fast,fast.encodedLength).method
  };
  const report={metadataBlock:live.blockHash,proposalHash:hash,proposalLength:len,wasmBlake2:blake2AsHex(wasm,256),councilCount,minimumCouncilThreshold:minimum,conservativeCouncilThreshold:conservative,technicalCommitteeCount:techCount,technicalCommitteeThreshold:techThreshold,votingPeriodBlocks:live.fastTrackVotingPeriod,delayBlocks:0,calls:{},submittedTransactions:0};
  for(const [name,value] of Object.entries(calls)){
   const encoded=value.toHex();
   writeFileSync(path.join(root,name+'.hex'),encoded+'\n');
   const decoded=pinnedRegistry.createType('Call',encoded);
   assert.equal(decoded.toHex(),encoded);
   assert.equal(decoded.section,value.section);
   assert.equal(decoded.method,value.method);
   report.calls[name]={file:name+'.hex',length:value.encodedLength,hash:blake2AsHex(value.toU8a(),256),section:value.section,method:value.method};
  }
  writeFileSync(path.join(root,'governance-calls.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
 } finally {await api.disconnect();clearTimeout(timeout)}
}
main().catch(e=>{console.error(e);process.exit(1)});
