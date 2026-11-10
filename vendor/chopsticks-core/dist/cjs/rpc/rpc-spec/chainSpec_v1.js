"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: Object.getOwnPropertyDescriptor(all, name).get
    });
}
_export(exports, {
    get chainSpec_v1_chainName () {
        return chainSpec_v1_chainName;
    },
    get chainSpec_v1_genesisHash () {
        return chainSpec_v1_genesisHash;
    },
    get chainSpec_v1_properties () {
        return chainSpec_v1_properties;
    }
});
const _shared = require("../shared.js");
const chainSpec_v1_chainName = async (context)=>{
    return context.chain.api.getSystemChain();
};
const chainSpec_v1_genesisHash = async (context)=>{
    const genesisHash = await context.chain.api.getBlockHash(0);
    if (genesisHash === null) {
        throw new _shared.ResponseError(1, 'Unexpected null genesis hash');
    }
    return genesisHash;
};
const chainSpec_v1_properties = async (context)=>{
    return context.chain.api.getSystemProperties();
};
