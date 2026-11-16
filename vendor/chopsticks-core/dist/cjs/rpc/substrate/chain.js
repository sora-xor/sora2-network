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
    get chain_getBlock () {
        return chain_getBlock;
    },
    get chain_getBlockHash () {
        return chain_getBlockHash;
    },
    get chain_getFinalizedHead () {
        return chain_getFinalizedHead;
    },
    get chain_getHead () {
        return chain_getHead;
    },
    get chain_getHeader () {
        return chain_getHeader;
    },
    get chain_subscribeFinalizedHeads () {
        return chain_subscribeFinalizedHeads;
    },
    get chain_subscribeNewHead () {
        return chain_subscribeNewHead;
    },
    get chain_subscribeNewHeads () {
        return chain_subscribeNewHeads;
    },
    get chain_unsubscribeFinalizedHeads () {
        return chain_unsubscribeFinalizedHeads;
    },
    get chain_unsubscribeNewHead () {
        return chain_unsubscribeNewHead;
    },
    get chain_unsubscribeNewHeads () {
        return chain_unsubscribeNewHeads;
    }
});
const _util = require("@polkadot/util");
const _shared = require("../shared.js");
const processHeader = ({ parentHash, number, stateRoot, extrinsicsRoot, digest })=>{
    return {
        parentHash: parentHash.toHex(),
        number: number.toHex(),
        stateRoot: stateRoot.toHex(),
        extrinsicsRoot: extrinsicsRoot.toHex(),
        digest: {
            logs: digest.logs.map((log)=>log.toHex())
        }
    };
};
const chain_getBlockHash = async (context, [blockNumber])=>{
    const numbers = Array.isArray(blockNumber) ? blockNumber : [
        blockNumber
    ];
    const hashes = await Promise.all(numbers.map((n)=>(0, _util.isHex)(n, undefined, true) ? (0, _util.hexToNumber)(n) : n).map((n)=>context.chain.getBlockAt(n))).then((blocks)=>blocks.map((b)=>b?.hash || null));
    return Array.isArray(blockNumber) ? hashes : hashes[0];
};
const chain_getHeader = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    return processHeader(await block.header);
};
const chain_getBlock = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    return {
        block: {
            header: processHeader(await block.header),
            extrinsics: await block.extrinsics
        },
        justifications: null
    };
};
const chain_getFinalizedHead = async (context)=>{
    return context.chain.head.hash;
};
const chain_subscribeNewHead = async (context, _params, { subscribe })=>{
    let update = ()=>{};
    const id = context.chain.headState.subscribeHead(()=>update());
    const callback = subscribe('chain_newHead', id, ()=>context.chain.headState.unsubscribeHead(id));
    update = async ()=>{
        callback(processHeader(await context.chain.head.header));
    };
    setTimeout(update, 50);
    return id;
};
const chain_subscribeFinalizedHeads = async (context, _params, { subscribe })=>{
    let update = ()=>{};
    const id = context.chain.headState.subscribeHead(()=>update());
    const callback = subscribe('chain_finalizedHead', id, ()=>context.chain.headState.unsubscribeHead(id));
    update = async ()=>{
        callback(processHeader(await context.chain.head.header));
    };
    setTimeout(update, 50);
    return id;
};
const chain_unsubscribeNewHead = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
const chain_getHead = chain_getBlockHash;
const chain_subscribeNewHeads = chain_subscribeNewHead;
const chain_unsubscribeNewHeads = chain_unsubscribeNewHead;
const chain_unsubscribeFinalizedHeads = chain_unsubscribeNewHead;
