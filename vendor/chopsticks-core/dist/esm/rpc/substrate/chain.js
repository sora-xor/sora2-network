import { hexToNumber, isHex } from '@polkadot/util';
import { ResponseError } from '../shared.js';
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
/**
 * @param context
 * @param params - [`blockNumber` | `blockNumber[]` | null]
 *
 * @return Block hash | hash[] | null
 */ export const chain_getBlockHash = async (context, [blockNumber])=>{
    const numbers = Array.isArray(blockNumber) ? blockNumber : [
        blockNumber
    ];
    const hashes = await Promise.all(numbers.map((n)=>isHex(n, undefined, true) ? hexToNumber(n) : n).map((n)=>context.chain.getBlockAt(n))).then((blocks)=>blocks.map((b)=>b?.hash || null));
    return Array.isArray(blockNumber) ? hashes : hashes[0];
};
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return Header - see `@polkadot/types/interfaces`
 */ export const chain_getHeader = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new ResponseError(1, `Block ${hash} not found`);
    }
    return processHeader(await block.header);
};
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return Block header and extrinsics
 */ export const chain_getBlock = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new ResponseError(1, `Block ${hash} not found`);
    }
    return {
        block: {
            header: processHeader(await block.header),
            extrinsics: await block.extrinsics
        },
        justifications: null
    };
};
/**
 * @param context
 *
 * @return head hash
 */ export const chain_getFinalizedHead = async (context)=>{
    return context.chain.head.hash;
};
export const chain_subscribeNewHead = async (context, _params, { subscribe })=>{
    let update = ()=>{};
    const id = context.chain.headState.subscribeHead(()=>update());
    const callback = subscribe('chain_newHead', id, ()=>context.chain.headState.unsubscribeHead(id));
    update = async ()=>{
        callback(processHeader(await context.chain.head.header));
    };
    setTimeout(update, 50);
    return id;
};
export const chain_subscribeFinalizedHeads = async (context, _params, { subscribe })=>{
    let update = ()=>{};
    const id = context.chain.headState.subscribeHead(()=>update());
    const callback = subscribe('chain_finalizedHead', id, ()=>context.chain.headState.unsubscribeHead(id));
    update = async ()=>{
        callback(processHeader(await context.chain.head.header));
    };
    setTimeout(update, 50);
    return id;
};
export const chain_unsubscribeNewHead = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
export const chain_getHead = chain_getBlockHash;
export const chain_subscribeNewHeads = chain_subscribeNewHead;
export const chain_unsubscribeNewHeads = chain_unsubscribeNewHead;
export const chain_unsubscribeFinalizedHeads = chain_unsubscribeNewHead;
