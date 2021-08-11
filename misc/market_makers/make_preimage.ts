
import { ApiPromise } from '@polkadot/api';
import { WsProvider } from '@polkadot/rpc-provider';
import { options } from '@sora-substrate/api';
import { Keyring } from '@polkadot/api';
import fs from 'fs';

// RUN VIA:
// cd misc/market_makers/
// yarn install
// ts-node make_preimage.ts

// select desired chain and snapshot

// const ENDPOINT = 'wss://mof3.sora.org';
const ENDPOINT = 'wss://ws.stage.sora2.soramitsu.co.jp/';

const SNAPSHOT = './market_makers_may_snapshot.json';
// const SNAPSHOT = './market_makers_june_snapshot.json';
// const SNAPSHOT = './stage_market_makers_may_snapshot.json';
// const SNAPSHOT = './stage_market_makers_june_snapshot.json';

async function main(): Promise<void> {
  const provider = new WsProvider(ENDPOINT);
  const api = new ApiPromise(options({ provider }));
  await api.isReady;

  const keyring = new Keyring({ type: 'sr25519' });
  const root = keyring.addFromMnemonic('era actor pluck voice frost club gallery palm moment empower whale flame');
  
  let elems: Array<Object> = Array();

  // read from file
  let file = fs.readFileSync(SNAPSHOT);
  let obj = JSON.parse(file.toString());
  
  // construct Vec
  for (let i = 0; i < obj.length; i++) {
    elems.push([obj[i].address, obj[i].count, obj[i].volume])
  }

  console.log("Total elems:", elems.length);

  // make tx
  let tx = api.tx.vestedRewards.injectMarketMakers(elems);

  let encodedPreimage = tx.method.toHex();
  let preimageTx = api.tx.democracy.notePreimage(encodedPreimage);

  await submitExtrinsic(api, preimageTx, root, "Submit preimage");
}

async function inner_submitExtrinsic(api: ApiPromise, extrinsic: any, signer: any, finishCallback: any): Promise<void> {
  const signedTx = await extrinsic.signAsync(signer);
  console.log("TX HASH:",signedTx.hash.toString());

  const unsub = await signedTx.send((result: any) => {
    console.log(`Current status is ${result.status}`);
    console.log(result.toString());

    if (result.status.isInBlock) {
      console.log(`Transaction included at blockHash ${result.status.asInBlock}`);
    } else if (result.status.isFinalized) {
      console.log(`Transaction finalized at blockHash ${result.status.asFinalized}`);

      result.events.forEach(({ phase, event: { data, method, section } }: any) => {
        console.log(`\t' ${phase}: ${section}.${method}:: ${data}`);
        if (section === 'system' && method === 'ExtrinsicFailed') {
          const [error,] = data;
          if (error.isModule) {
            const decoded = api.registry.findMetaError(error.asModule);
            const { documentation, name, section } = decoded;
            console.log(`${section}.${name}: ${documentation.join(' ')}`);
          } else {
            // Other, CannotLookup, BadOrigin, no extra info
            console.log(error.toString());
          }
        }
      });

      unsub();
      finishCallback();
    }
  });
}

async function submitExtrinsic(api: ApiPromise, extrinsic: any, signer: any, debugMessage = ''): Promise<void> {
  console.log(`\nSubmit extrinsic: ${debugMessage}\n`);
  return new Promise((resolve, _reject) => {
    inner_submitExtrinsic(api, extrinsic, signer, resolve);
  });
}

main().catch(console.error).finally(() => process.exit());