// Synthetic codec boundary tests; actual metadata/capture is checked by the runner.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {networkIdentity,networkComponent,decodeStorageKey,COUNTS,CONFIG} from './inventory_checkpoint_bridge_sources.mjs';
const hx=b=>'0x'+Buffer.from(b).toString('hex');
function generic(raw){
  const tag=raw[0];if(tag===0)return {consumed:33,value:{variant:'EVM',fields:[]}};
  if(tag===2)return {consumed:5,value:{variant:'EVMLegacy',fields:[]}};
  return {consumed:2,value:{variant:tag===1?'Sub':'TON',fields:[{variant:tag===1?'Kusama':'Mainnet',fields:[]}]}};
}
test('internal HASHI network IDs never become inferred external chain IDs',()=>{
  const id=networkIdentity(Buffer.from([2,1,0,0,0]),generic);
  assert.equal(id.family,'hashi');assert.equal(id.internalNetworkId,1);assert.equal(id.externalEvmChainId,null);
  const raw=Buffer.alloc(33);raw[32]=1;const evm=networkIdentity(raw,generic);
  assert.equal(evm.family,'generic-evm');assert.equal(evm.externalEvmChainId,'1');assert.equal(evm.productionAppFeatureGate,'wip');
  assert.notEqual(evm.genericNetworkScale,id.genericNetworkScale);
});
test('Substrate, TON and HASHI storage keys use their exact distinct network namespaces',()=>{
  const id=Buffer.from([1,0,0,0]);assert.equal(networkComponent('EthBridge.Requests',[id],Buffer.alloc(0),generic).genericNetworkScale,'0x0201000000');
  assert.equal(networkComponent('SubstrateBridgeInboundChannel.ChannelNonces',[Buffer.from([1])],Buffer.alloc(0),generic).family,'substrate');
  assert.equal(networkComponent('BridgeInboundChannel.TONChannelAddresses',[Buffer.from([0])],Buffer.alloc(0),generic).family,'ton');
  assert.equal(networkComponent('JettonApp.AppInfo',[],Buffer.from([0,...Array(33).fill(8)]),generic).genericNetworkScale,'0x0300');
  assert.equal(networkComponent('JettonApp.TokenAddresses',[Buffer.alloc(32)],Buffer.alloc(33),generic),null);
  assert.equal(networkComponent('EthBridge.XorMasterContractAddress',[],Buffer.alloc(20),generic),null);
});
test('BridgeProxy tuple keys preserve account suffixes without treating them as network bytes',()=>{
  const raw=Buffer.concat([Buffer.from([1,1]),Buffer.alloc(32,8)]);
  const id=networkComponent('BridgeProxy.Transactions',[raw,Buffer.alloc(32)],Buffer.alloc(0),generic);
  assert.equal(id.genericNetworkScale,'0x0101');assert.equal(id.network,'Kusama');
  assert.throws(()=>networkIdentity(Buffer.from([2,0]),generic),/width/);
  assert.throws(()=>networkIdentity(Buffer.from([4,0]),()=>({consumed:2,value:{variant:'Unknown'}})),/unsupported/);
});
test('reversible key components require exact hash, width and complete consumption',()=>{
  const prefix='0x'+'11'.repeat(32),first=Buffer.from([2,1,0,0,0]),second=Buffer.alloc(32,3);
  const hash=(_h,b)=>createHash('sha256').update(b).digest().subarray(0,8); // Synthetic hasher only.
  const descriptor={prefix,hashers:['Twox64Concat','Identity'],keyTypes:[1,2]};
  const parse=(b,type)=>({consumed:type===1?5:32,value:hx(b.subarray(0,type===1?5:32))});
  const hashComponent=(h,b)=>h==='Identity'?Buffer.alloc(0):hash(h,b);
  const key=prefix+hx(Buffer.concat([hash('',first),first,second])).slice(2);
  assert.deepEqual(decodeStorageKey(key,descriptor,parse,hashComponent).components,[first,second]);
  assert.throws(()=>decodeStorageKey(key+'00',descriptor,parse,hashComponent),/trailing/);
  assert.throws(()=>decodeStorageKey(prefix+'00'+key.slice(prefix.length+2),descriptor,parse,hashComponent),/hash/);
  assert.throws(()=>decodeStorageKey(key,{...descriptor,hashers:['Blake2_256','Identity']},parse,hashComponent),/reversible/);
});
test('registry descriptors include explicit absent candidates and no duplicated counting/configuration roles',()=>{
  assert.equal(new Set([...CONFIG,...COUNTS]).size,CONFIG.length+COUNTS.length);
  for(const name of ['MultisigVerifier.PeerKeys','BridgeDataSigner.Peers','BridgeInboundChannel.EVMChannelAddresses','BridgeInboundChannel.TONChannelAddresses','JettonApp.AppInfo'])assert.ok(CONFIG.includes(name));
  for(const name of ['EthBridge.Requests','BridgeProxy.Senders','BridgeProxy.Transactions'])assert.ok(COUNTS.includes(name));
});
