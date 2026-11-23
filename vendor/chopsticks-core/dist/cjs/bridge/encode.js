// Value encoders for `pallet_bridge_parachains` storage items. The bridge connector forges
// these values via `dev_setStorage`, so they're SCALE-encoded by hand — runtime metadata
// describes their types but can't construct them. Storage *keys*, by contrast, are derived
// from metadata by the callers (`api.query[pallet].item.key(...)`), never hand-hashed.
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
    get encodeParaInfo () {
        return encodeParaInfo;
    },
    get encodeParaStoredHeaderData () {
        return encodeParaStoredHeaderData;
    }
});
const _util = require("@polkadot/util");
const u32LE = (v)=>(0, _util.nToU8a)(v, {
        bitLength: 32,
        isLe: true
    });
const requireBytes = (label, b, len)=>{
    if (b.length !== len) throw new Error(`${label} must be ${len} bytes, got ${b.length}`);
};
const encodeParaInfo = (atRelayBlockNumber, headHash, nextImportedHashPosition)=>{
    requireBytes('headHash', headHash, 32);
    return (0, _util.u8aToHex)((0, _util.u8aConcat)(u32LE(atRelayBlockNumber), headHash, u32LE(nextImportedHashPosition)));
};
const encodeParaStoredHeaderData = (blockNumber, stateRoot)=>{
    requireBytes('stateRoot', stateRoot, 32);
    return (0, _util.u8aToHex)((0, _util.compactAddLength)((0, _util.u8aConcat)(u32LE(blockNumber), stateRoot)));
};
