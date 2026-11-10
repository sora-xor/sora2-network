import { defaultLogger } from '../logger.js';
import { getParaId } from '../utils/index.js';
import { connectDownward } from './downward.js';
import { connectHorizontal } from './horizontal.js';
import { connectUpward } from './upward.js';
export const xcmLogger = defaultLogger.child({
    name: 'xcm'
});
export const connectVertical = async (relaychain, parachain)=>{
    await connectDownward(relaychain, parachain);
    await connectUpward(parachain, relaychain);
    xcmLogger.info(`Connected relaychain '${await relaychain.api.getSystemChain()}' with parachain '${await parachain.api.getSystemChain()}'`);
};
export const connectParachains = async (parachains, disableAutoHrmp = false)=>{
    const list = {};
    for (const chain of parachains){
        const paraId = await getParaId(chain);
        list[paraId.toNumber()] = chain;
    }
    await connectHorizontal(list, disableAutoHrmp);
    xcmLogger.info(`Connected parachains [${Object.keys(list)}]`);
};
