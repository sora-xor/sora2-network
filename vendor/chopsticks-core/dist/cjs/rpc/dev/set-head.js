"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_setHead", {
    enumerable: true,
    get: function() {
        return dev_setHead;
    }
});
const _zod = require("zod");
const _shared = require("../shared.js");
const schema = _shared.zHash.or(_zod.z.number());
const dev_setHead = async (context, [params])=>{
    const hashOrNumber = schema.parse(params);
    let block;
    if (typeof hashOrNumber === 'number') {
        const blockNumber = hashOrNumber > 0 ? hashOrNumber : context.chain.head.number + hashOrNumber;
        block = await context.chain.getBlockAt(blockNumber);
    } else {
        block = await context.chain.getBlock(hashOrNumber);
    }
    if (!block) {
        throw new _shared.ResponseError(1, `Block not found ${hashOrNumber}`);
    }
    await context.chain.setHead(block);
    return block.hash;
};
