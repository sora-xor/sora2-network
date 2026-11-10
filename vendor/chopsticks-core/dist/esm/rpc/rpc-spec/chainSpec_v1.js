import { ResponseError } from '../shared.js';
export const chainSpec_v1_chainName = async (context)=>{
    return context.chain.api.getSystemChain();
};
export const chainSpec_v1_genesisHash = async (context)=>{
    const genesisHash = await context.chain.api.getBlockHash(0);
    if (genesisHash === null) {
        throw new ResponseError(1, 'Unexpected null genesis hash');
    }
    return genesisHash;
};
export const chainSpec_v1_properties = async (context)=>{
    return context.chain.api.getSystemProperties();
};
