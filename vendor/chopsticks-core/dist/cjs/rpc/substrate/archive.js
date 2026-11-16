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
    get archive_unstable_body () {
        return archive_unstable_body;
    },
    get archive_unstable_call () {
        return archive_unstable_call;
    },
    get archive_unstable_hashByHeight () {
        return archive_unstable_hashByHeight;
    }
});
const _shared = require("../shared.js");
const _chain = require("./chain.js");
const archive_unstable_body = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    return await block.extrinsics;
};
const archive_unstable_call = async (context, [hash, method, data])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    const resp = await block.call(method, [
        data
    ]);
    return {
        success: true,
        value: resp.result
    };
};
const archive_unstable_hashByHeight = _chain.chain_getBlockHash;
