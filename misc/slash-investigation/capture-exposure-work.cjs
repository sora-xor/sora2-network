const {createRequire} = require('node:module');
const {writeFileSync} = require('node:fs');
const deps = createRequire('/tmp/sora-payout-validation/package.json');
const {ApiPromise, WsProvider} = deps('@polkadot/api');
const {fullOverrideBundle} = deps('@sora-substrate/type-definitions');
const deadline = setTimeout(() => process.exit(2), 60000);
(async () => {
 const endpoint = 'wss://ws.mof.sora.org';
 const api = await ApiPromise.create({provider: new WsProvider(endpoint), typesBundle: {spec: {'sora-substrate': fullOverrideBundle.spec.sora}}});
 try {
  if(api.genesisHash.toHex() !== '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5') throw new Error('Unexpected chain');
  const hash=await api.rpc.chain.getFinalizedHead(); const at=await api.at(hash);
  const bonded=(await at.query.staking.bondedEras()).toJSON();
  const eras=[];
  for(let i=0;i<bonded.length;i+=5) {
   eras.push(...await Promise.all(bonded.slice(i,i+5).map(async ([era,session])=>{
    const overviews=await at.query.staking.erasStakersOverview.entries(era);
    const paged=new Set(); let total=0; let largest=0;
    for(const [key,value] of overviews) {
      const o=value.unwrap(); const n=o.nominatorCount.toNumber();
      paged.add(key.args[1].toString()); total+=n; largest=Math.max(largest,n);
    }
    const legacy=await at.query.staking.erasStakers.entries(era);
    let legacyOnly=0;
    for(const [key,value] of legacy) if(!paged.has(key.args[1].toString())) {
      const n=value.others.length; total+=n; largest=Math.max(largest,n); legacyOnly++;
    }
    return {era,session,pagedValidators:overviews.length,legacyValidators:legacy.length,legacyOnlyValidators:legacyOnly,totalNominationRows:total,largestValidatorNominationRows:largest};
   })));
  }
  const result={checkedAt:new Date().toISOString(),endpoint,blockHash:hash.toHex(),blockNumber:(await api.rpc.chain.getHeader(hash)).number.toNumber(),activeEra:(await at.query.staking.activeEra()).toJSON(),eras,submittedTransactions:0};
  writeFileSync('/Users/takemiyamakoto/dev/sora2-network/misc/slash-investigation/exposure-work-snapshot.json',JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify(result,null,2));
 } finally {await api.disconnect();clearTimeout(deadline);}
})().catch(error=>{console.error(error);process.exit(1)});
