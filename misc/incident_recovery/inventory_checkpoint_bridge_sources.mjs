#!/usr/bin/env node
/** Public source-registry inventory from authenticated checkpoint storage.
 * No RPC, external chain-ID inference, bridge activation or native execution. */
import {createReadStream} from 'node:fs';
import {readFile,writeFile,mkdir,lstat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {CHECKPOINT,MAX_PAGE_BYTES} from './capture_checkpoint.mjs';
import {parseTyped} from './independent_attacker_inventory.mjs';
import {selectedRows} from './prepare_scheduler_review.mjs';
import {streamDigest} from './checkpoint_inventory_catalog.mjs';
import {checkObservedPages} from './prepare_checkpoint_bridge_reviews.mjs';
const need=(v,m)=>{if(!v)throw Error(m);},sha=b=>createHash('sha256').update(b).digest('hex');
const hx=b=>'0x'+Buffer.from(b).toString('hex'),json=v=>Buffer.from(JSON.stringify(v,null,2)+'\n');
const bytes=v=>{need(typeof v==='string'&&/^0x(?:[0-9a-f]{2})*$/.test(v),'canonical hex required');return Buffer.from(v.slice(2),'hex');};
export const COUNTS=Object.freeze([
  'EthBridge.Requests','EthBridge.RequestStatuses','EthBridge.RequestApprovals','EthBridge.RequestApprovers',
  'EthBridge.RequestSubmissionHeight','EthBridge.LoadToIncomingRequestHash','EthBridge.RequestsQueue',
  'BridgeProxy.Senders','BridgeProxy.Transactions','BridgeDataSigner.Approvals','BridgeDataSigner.PendingApprovals',
  'BridgeOutboundChannel.LatestCommitment','BridgeOutboundChannel.MessageQueues',
  'SubstrateBridgeOutboundChannel.LatestCommitment','SubstrateBridgeOutboundChannel.MessageQueues',
]);
export const CONFIG=Object.freeze([
  ...['BridgeAccount','BridgeContractAddress','BridgeStatuses','Peers','PeerAddress','PeerAccountId','PendingPeer',
    'BridgeSignatureVersions','PendingBridgeSignatureVersions','NextNetworkId','XorMasterContractAddress','ValMasterContractAddress',
    'LegacyEthereumXorDecommissioned','LegacyEthereumXorDecommissionedAt','RegisteredAsset','RegisteredSidechainAsset',
    'RegisteredSidechainToken','SidechainAssetPrecision','DeprecatedSidechainTokens'].map(n=>'EthBridge.'+n),
  ...['ChannelNonces','ReportedChannelNonces','EVMChannelAddresses','TONChannelAddresses'].map(n=>'BridgeInboundChannel.'+n),
  'BridgeOutboundChannel.ChannelNonces','SubstrateBridgeInboundChannel.ChannelNonces','SubstrateBridgeOutboundChannel.ChannelNonces',
  'BridgeDataSigner.Peers','BridgeDataSigner.PendingPeerUpdate','MultisigVerifier.PeerKeys',
  ...['AppInfo','AssetKinds','TokenAddresses','AssetsByAddresses','SidechainPrecision'].map(n=>'JettonApp.'+n),
  ...['AssetKinds','SidechainPrecision','AllowedParachainAssets','RelaychainAsset'].map(n=>'ParachainBridgeApp.'+n),
  ...['AssetKinds','SidechainPrecision','SidechainAssetId','ThischainAssetId'].map(n=>'SubstrateBridgeApp.'+n),
]);

export function networkIdentity(encoded,decodeGeneric) {
  const parsed=decodeGeneric(encoded),raw=encoded.subarray(0,parsed.consumed),variant=parsed.value.variant;
  need(['EVM','EVMLegacy','Sub','TON'].includes(variant),'unsupported generic network identity');
  if(variant==='EVMLegacy') {
    need(raw.length===5,'HASHI internal network width');
    return {genericNetworkScale:hx(raw),family:'hashi',internalNetworkId:raw.readUInt32LE(1),externalEvmChainId:null,
      interpretation:'Internal HASHI network ID; this number does not establish an external EVM chain ID.'};
  }
  if(variant==='EVM') {
    need(raw.length===33,'generic EVM chain width');
    return {genericNetworkScale:hx(raw),family:'generic-evm',externalEvmChainId:BigInt(hx(raw.subarray(1))).toString(),
      chainIdH256:hx(raw.subarray(1)),productionAppFeatureGate:'wip',
      interpretation:'Generic EVM identity; source configuration does not prove that the WIP app is enabled.'};
  }
  const field=parsed.value.fields?.[0];need(raw.length===2&&field?.variant,'named Substrate/TON network required');
  return {genericNetworkScale:hx(raw),family:variant==='Sub'?'substrate':'ton',network:field.variant,
    interpretation:variant==='Sub'?'Protocol SubNetworkId; endpoint, genesis and para-chain identity require separate source verification.':'Protocol TON network; the channel/app address records are separate configuration evidence.'};
}
export function networkComponent(storage,components,value,decodeGeneric) {
  if(storage.startsWith('EthBridge.'))return components.length?networkIdentity(Buffer.concat([Buffer.from([2]),components[0]]),decodeGeneric):null;
  if(storage==='JettonApp.AppInfo')return networkIdentity(Buffer.concat([Buffer.from([3]),value.subarray(0,1)]),decodeGeneric);
  if(storage.startsWith('JettonApp.'))return null;
  if(storage==='BridgeInboundChannel.EVMChannelAddresses')return networkIdentity(Buffer.concat([Buffer.from([0]),components[0]]),decodeGeneric);
  if(storage==='BridgeInboundChannel.TONChannelAddresses')return networkIdentity(Buffer.concat([Buffer.from([3]),components[0]]),decodeGeneric);
  if(/^(SubstrateBridge(?:InboundChannel|OutboundChannel|App)|ParachainBridgeApp)\./.test(storage))
    return networkIdentity(Buffer.concat([Buffer.from([1]),components[0]]),decodeGeneric);
  return networkIdentity(components[0],decodeGeneric); // BridgeProxy.Transaction tuple begins with its network.
}
export function decodeStorageKey(key,descriptor,parse,hashComponent) {
  need(key.startsWith(descriptor.prefix),'storage prefix differs');const raw=bytes(key);let offset=bytes(descriptor.prefix).length;const components=[],decoded=[];
  for(const [i,hasher]of descriptor.hashers.entries()) {
    const width={Identity:0,Twox64Concat:8,Blake2_128Concat:16}[hasher];need(width!==undefined,'source registry requires reversible key components');
    need(offset+width<=raw.length,'truncated registry key');const part=raw.subarray(offset+width),value=parse(part,descriptor.keyTypes[i]);
    const encoded=part.subarray(0,value.consumed);need(Buffer.from(hashComponent(hasher,encoded)).equals(raw.subarray(offset,offset+width)),'source registry key hash differs');
    components.push(encoded);decoded.push(value.value);offset+=width+encoded.length;
  }
  need(offset===raw.length,'source registry key trailing bytes');return {components,decoded};
}

export async function main(args) {
  const o={};for(let i=0;i<args.length;i+=2){need(args[i]?.startsWith('--')&&args[i+1],'expected --name value');const k=args[i].slice(2);need(!o[k],'duplicate option');o[k]=resolve(args[i+1]);}
  const required=['capture','verification','metadata-witness','prepared-reviews','output'];
  need(required.every(k=>o[k])&&Object.keys(o).length===required.length,'required paths: '+required.join(', '));
  const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),pins=new Map(),pagePins=new Map();
  async function load(path){const stat=await lstat(path);need(stat.isFile()&&!stat.isSymbolicLink()&&stat.size<=64*1024**2,`bounded regular input required: ${path}`);const b=await readFile(path);pins.set(path,sha(b));return b;}
  const manifestRaw=await load(join(o.capture,'capture-complete.json')),manifest=JSON.parse(manifestRaw);
  const verificationRaw=await load(o.verification),verification=JSON.parse(verificationRaw),witnessRaw=await load(o['metadata-witness']),witness=JSON.parse(witnessRaw);
  const generation=JSON.parse(await load(join(o['prepared-reviews'],'generation.json'))),prepared={};
  need(generation.schema==='sora-nexus-automatic-checkpoint-reviews-v1'&&generation.productionReady===false,'source-derived generation required');
  for(const file of ['evidence.json','prefix-catalog.json','observed-page-ranges.json']){const raw=await load(join(o['prepared-reviews'],file));need(generation.outputs[file]?.sha256===sha(raw),'prepared output differs');prepared[file]=JSON.parse(raw);}
  const evidence=prepared['evidence.json'],catalog=prepared['prefix-catalog.json'],observed=prepared['observed-page-ranges.json'];
  need(manifest.complete===true&&manifest.children.length===0&&manifest.checkpoint.hash===CHECKPOINT.hash&&manifest.checkpoint.stateRoot===CHECKPOINT.stateRoot
    &&verification.stateRootVerified===true&&verification.headerHashVerified===true&&verification.checkpointHash===CHECKPOINT.hash
    &&verification.stateRoot===CHECKPOINT.stateRoot&&verification.captureManifestSha256===sha(manifestRaw),'root-verified checkpoint required');
  need(evidence.inputSha256.checkpointManifest===sha(manifestRaw)&&evidence.inputSha256.verification===sha(verificationRaw)
    &&evidence.inputSha256['metadata-witness']===sha(witnessRaw)&&evidence.completePrefixCatalogSha256===sha(json(catalog))
    &&evidence.observedPageRangesSha256===sha(json(observed))&&evidence.pageRangeHintsTrusted===false,'observed range evidence differs');
  checkObservedPages(manifest,observed,catalog);
  const metadataRaw=await load(join(o.capture,'runtime-metadata.scale'));
  need(witness.wasmExecutionVerified===true&&witness.nativeExecution===false&&witness.checkpointHash===CHECKPOINT.hash
    &&witness.stateRoot===CHECKPOINT.stateRoot&&witness.metadataSha256===sha(metadataRaw),'executed metadata differs');
  const dependency=join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'),require=createRequire(dependency);
  const {Metadata,TypeRegistry}=require('@polkadot/types'),{blake2AsU8a,xxhashAsU8a}=require('@polkadot/util-crypto');
  const hash=b=>hx(blake2AsU8a(b,256)),registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,metadataRaw);registry.setMetadata(metadata);const portable=metadata.asLatest.toJSON(),types=new Map(portable.lookup.types.map(t=>[t.id,t.type]));
  const generic=[...types].find(([,t])=>t.path?.at(-1)==='GenericNetworkId');need(generic,'generic network metadata missing');
  const parse=(b,id)=>parseTyped(b,id,types,{render:true,watched:new Set()}),decodeGeneric=b=>parse(b,generic[0]);
  const descriptors=new Map(),names=[...COUNTS,...CONFIG];
  for(const pallet of portable.pallets)if(pallet.storage)for(const entry of pallet.storage.items){
    const name=`${pallet.name}.${entry.name}`;if(!names.includes(name))continue;const map=entry.type.map;
    const keyTypes=map?(map.hashers.length===1?[map.key]:types.get(map.key)?.def.tuple):[];
    need(!map||keyTypes?.length===map.hashers.length,'registry key tuple differs');
    const prefix=hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(entry.name,128)]));
    descriptors.set(prefix,{name,prefix,keyTypes,hashers:map?.hashers??[],valueType:map?.value??entry.type.plain});
  }
  need(descriptors.size===names.length,'checkpoint source-registry descriptors incomplete');
  const hashComponent=(h,b)=>h==='Identity'?Buffer.alloc(0):h==='Twox64Concat'?xxhashAsU8a(b,64):blake2AsU8a(b,128);
  const configuration=[],networks=new Map(),counts=new Map(names.map(n=>[n,0])),prefixHeads=new Map(names.map(n=>[n,hash(Buffer.from('sora-nexus-source-registry-v1:'+n))]));
  const {compactToU8a}=require('@polkadot/util');let scanned=0;
  for(const page of observed){
    const prefixes=[...descriptors.keys()].filter(p=>page.keyCount&&page.firstKey<=p+'f'.repeat(1024)&&page.lastKey>=p);if(!prefixes.length)continue;
    const path=join(o.capture,page.file),digest=await streamDigest(path,MAX_PAGE_BYTES);need(digest===page.sha256,'registry page checksum differs');pagePins.set(path,digest);
    for await(const [key,value]of selectedRows(createReadStream(path).pipe(createGunzip()),prefixes)) {
      const d=descriptors.get(key.slice(0,66));need(d,'unknown selected registry prefix');const raw=bytes(value),decoded=decodeStorageKey(key,d,parse,hashComponent);
      const network=networkComponent(d.name,decoded.components,raw,decodeGeneric),valueHash=hash(raw),keyBytes=bytes(key);
      prefixHeads.set(d.name,hash(Buffer.concat([bytes(prefixHeads.get(d.name)),compactToU8a(keyBytes.length),keyBytes,bytes(valueHash)])));
      counts.set(d.name,counts.get(d.name)+1);need(++scanned<=1500000,'source registry row bound');
      if(network){const item=networks.get(network.genericNetworkScale)??{...network,observedRecords:{},configurationRows:[]};
        item.observedRecords[d.name]=(item.observedRecords[d.name]??0)+1;if(CONFIG.includes(d.name))item.configurationRows.push(configuration.length);networks.set(network.genericNetworkScale,item);}
      if(CONFIG.includes(d.name)){
        const parsed=parse(raw,d.valueType);need(parsed.consumed===raw.length&&Buffer.from(registry.createTypeUnsafe(`Lookup${d.valueType}`,[raw]).toU8a()).equals(raw),'registry configuration codec differs');
        configuration.push({storage:d.name,key,keyComponents:decoded.decoded,valueScale:value,valueDecoded:parsed.value,valueHash,network:network?.genericNetworkScale??null});
        need(configuration.length<=10000,'source registry configuration bound');
      }
    }
  }
  for(const name of names)need(counts.get(name)===(catalog.find(r=>r.storage===name)?.entries??0),`incomplete source registry prefix ${name}`);
  const sourceNames=['misc/incident_recovery/inventory_checkpoint_bridge_sources.mjs','misc/incident_recovery/prepare_checkpoint_bridge_reviews.mjs','misc/incident_recovery/checkpoint_inventory_catalog.mjs',
    'misc/incident_recovery/prepare_scheduler_review.mjs','misc/incident_recovery/independent_attacker_inventory.mjs','misc/incident_recovery/capture_checkpoint.mjs','runtime/src/lib.rs','pallets/eth-bridge/src/lib.rs','pallets/bridge-proxy/src/lib.rs',
    ...['types/src/lib.rs','types/src/ton.rs','channel/src/inbound/mod.rs','channel/src/outbound/mod.rs','substrate-channel/src/inbound/mod.rs','substrate-channel/src/outbound/mod.rs','parachain-app/src/lib.rs','substrate-app/src/lib.rs','jetton-app/src/lib.rs','data-signer/src/lib.rs','multisig-verifier/src/lib.rs'].map(p=>'vendor/sora2-common/pallets/'+p),
    'runtime-rehearsal-async-staking-2026-10-25/validation/package.json','runtime-rehearsal-async-staking-2026-10-25/validation/package-lock.json'];
  const sources=[];for(const file of sourceNames)sources.push({file,sha256:sha(await load(join(root,file)))});
  for(const[path,digest]of pins)need(sha(await readFile(path))===digest,'registry input/source changed');
  for(const[path,digest]of pagePins)need(await streamDigest(path,MAX_PAGE_BYTES)===digest,'registry capture page changed');
  const report={schema:'sora-nexus-checkpoint-bridge-source-registry-v1',checkpointHash:CHECKPOINT.hash,stateRoot:CHECKPOINT.stateRoot,
    captureManifestSha256:sha(manifestRaw),stateVerificationSha256:sha(verificationRaw),executingMetadataWitnessSha256:sha(witnessRaw),preparedEvidenceSha256:sha(json(evidence)),
    sources,pages:[...pagePins].map(([path,sha256])=>({file:path.slice(o.capture.length+1),sha256})),
    networks:[...networks.values()].sort((a,b)=>a.genericNetworkScale.localeCompare(b.genericNetworkScale)),configuration,
    prefixes:names.map(storage=>({storage,records:counts.get(storage),commitment:prefixHeads.get(storage)})),
    metadataHasGenericEvmFungibleApp:portable.pallets.some(p=>p.name==='EVMFungibleApp'),
    networkActivityEstablished:false,sourceChainFinalityVerified:false,sourceEndpointsSelected:false,remoteRequests:0,submittedTransactions:0,productionReady:false,
    limitations:'Checkpoint configuration and request-bearing identities only. Nonces/peer sets do not prove current activity, signing-key control, counterpart finality or safe incoming completeness. HASHI internal IDs never substitute for external chain IDs. Data-signer and inbound-verifier peer sets remain separate. Jetton asset rows are global to AppInfo; missing AppInfo is not synthesized.'};
  await mkdir(o.output,{recursive:false});await writeFile(join(o.output,'report.json'),json(report),{flag:'wx'});
  process.stdout.write(JSON.stringify({output:join(o.output,'report.json'),networks:report.networks.length,configurationRows:configuration.length,scannedRecords:scanned,productionReady:false})+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main(process.argv.slice(2)).catch(error=>{console.error(error.message);process.exitCode=1;});
