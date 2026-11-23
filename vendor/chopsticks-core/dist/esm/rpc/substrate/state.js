import { stringToHex } from '@polkadot/util';
import { defaultLogger } from '../../logger.js';
import { isPrefixedChildKey, prefixedChildKey, stripChildPrefix } from '../../utils/index.js';
import { buildReadProof } from '../read-proof.js';
import { ResponseError } from '../shared.js';
const logger = defaultLogger.child({
    name: 'rpc-state'
});
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return runtime version
 */ export const state_getRuntimeVersion = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    return block?.runtimeVersion || null;
};
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return metadata
 */ export const state_getMetadata = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    return block?.metadata || null;
};
/**
 * @param context
 * @param params - [`key`, `blockhash`]
 *
 * @return storage value
 */ export const state_getStorage = async (context, [key, hash])=>{
    const block = await context.chain.getBlock(hash);
    const value = await block?.get(key) || null;
    return value || null;
};
/**
 * @param context
 * @param params - [`prefix`, `pageSize`, `startKey`, `blockhash`]
 *
 * @return paged keys
 */ export const state_getKeysPaged = async (context, [prefix, pageSize, startKey, hash])=>{
    const block = await context.chain.getBlock(hash);
    return block?.getKeysPaged({
        prefix,
        pageSize,
        startKey
    });
};
/**
 * @param context
 * @param params - [`keys`, `blockhash`]
 *
 * @return storage values
 */ export const state_queryStorageAt = async (context, [keys, hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        return [];
    }
    const values = await Promise.all(keys.map(async (key)=>[
            key,
            await block.get(key).then((val)=>val || null)
        ]));
    return [
        {
            block: block.hash,
            changes: values
        }
    ];
};
/**
 * @param context
 * @param params - [`method`, `data`, `blockhash`]
 *
 * @return result in hash
 */ export const state_call = async (context, [method, data, hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new ResponseError(1, `Block ${hash} not found`);
    }
    const resp = await block.call(method, [
        data
    ]);
    return resp.result;
};
/**
 * @param context
 * @param params - [`keys`, `blockhash?`]
 *
 * @return `{ at, proof }` — proof is a list of SCALE-encoded trie nodes.
 *
 * Spec-compatible response shape. The composed proof's root diverges from
 * `chain_getHeader(at).state_root` once local overrides are applied (see `buildReadProof`);
 * callers that need the recomputed root should use chopsticks's `dev_getReadProof`.
 * Child-storage keys are rejected — use `state_getChildReadProof`.
 */ export const state_getReadProof = async (context, [keys, hash])=>{
    const { at, proof } = await buildReadProof(context, keys, hash);
    return {
        at,
        proof
    };
};
/**
 * @return subscription id
 */ export const state_subscribeRuntimeVersion = async (context, _params, { subscribe })=>{
    let update = (_block)=>{};
    const id = await context.chain.headState.subscribeStorage([
        stringToHex(':code')
    ], (block)=>update(block));
    const callback = subscribe('state_runtimeVersion', id, ()=>context.chain.headState.unsubscribeStorage(id));
    update = async (block)=>callback(await block.runtimeVersion);
    (async ()=>{
        update(context.chain.head);
    })();
    return id;
};
/**
 * @param context
 * @param params - [`subid`]
 * @param subscriptionManager
 */ export const state_unsubscribeRuntimeVersion = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
/**
 * @param context
 * @param params - [`keys`]
 * @param subscriptionManager
 *
 * @return subscription id
 */ export const state_subscribeStorage = async (context, [keys], { subscribe })=>{
    let update = (_block, _pairs)=>{};
    const id = await context.chain.headState.subscribeStorage(keys, (block, pairs)=>update(block, pairs));
    const callback = subscribe('state_storage', id, ()=>context.chain.headState.unsubscribeStorage(id));
    update = async (block, pairs)=>{
        logger.trace({
            hash: block.hash
        }, 'state_subscribeStorage');
        callback({
            block: block.hash,
            changes: pairs
        });
    };
    (async ()=>{
        const pairs = await Promise.all(keys.map(async (key)=>{
            const val = await context.chain.head.get(key);
            return [
                key,
                val || null
            ];
        }));
        callback({
            block: context.chain.head.hash,
            changes: pairs
        });
    })();
    return id;
};
/**
 * @param context
 * @param params - [`subid`]
 * @param subscriptionManager
 */ export const state_unsubscribeStorage = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
/**
 * @param context
 * @param params - [`child`, `key`, `blockhash`]
 *
 * @return storage valuse
 */ export const childstate_getStorage = async (context, [child, key, hash])=>{
    if (!isPrefixedChildKey(child)) {
        throw new ResponseError(-32000, 'Client error: Invalid child storage key');
    }
    const block = await context.chain.getBlock(hash);
    const value = await block?.get(prefixedChildKey(child, key));
    return value || null;
};
/**
 * @param context
 * @param params - [`child`, `prefix`, `pageSize`, `startKey`, `blockhash`]
 *
 * @return paged keys
 */ export const childstate_getKeysPaged = async (context, [child, prefix, pageSize, startKey, hash])=>{
    if (!isPrefixedChildKey(child)) {
        throw new ResponseError(-32000, 'Client error: Invalid child storage key');
    }
    const block = await context.chain.getBlock(hash);
    return block?.getKeysPaged({
        prefix: prefixedChildKey(child, prefix),
        pageSize,
        startKey: prefixedChildKey(child, startKey)
    }).then((keys)=>keys.map(stripChildPrefix));
};
export const state_getStorageAt = state_getStorage;
