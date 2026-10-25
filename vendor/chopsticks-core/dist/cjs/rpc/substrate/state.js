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
    get childstate_getKeysPaged () {
        return childstate_getKeysPaged;
    },
    get childstate_getStorage () {
        return childstate_getStorage;
    },
    get state_call () {
        return state_call;
    },
    get state_getKeysPaged () {
        return state_getKeysPaged;
    },
    get state_getMetadata () {
        return state_getMetadata;
    },
    get state_getReadProof () {
        return state_getReadProof;
    },
    get state_getRuntimeVersion () {
        return state_getRuntimeVersion;
    },
    get state_getStorage () {
        return state_getStorage;
    },
    get state_getStorageAt () {
        return state_getStorageAt;
    },
    get state_queryStorageAt () {
        return state_queryStorageAt;
    },
    get state_subscribeRuntimeVersion () {
        return state_subscribeRuntimeVersion;
    },
    get state_subscribeStorage () {
        return state_subscribeStorage;
    },
    get state_unsubscribeRuntimeVersion () {
        return state_unsubscribeRuntimeVersion;
    },
    get state_unsubscribeStorage () {
        return state_unsubscribeStorage;
    }
});
const _util = require("@polkadot/util");
const _logger = require("../../logger.js");
const _index = require("../../utils/index.js");
const _readproof = require("../read-proof.js");
const _shared = require("../shared.js");
const logger = _logger.defaultLogger.child({
    name: 'rpc-state'
});
const state_getRuntimeVersion = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    return block?.runtimeVersion || null;
};
const state_getMetadata = async (context, [hash])=>{
    const block = await context.chain.getBlock(hash);
    return block?.metadata || null;
};
const state_getStorage = async (context, [key, hash])=>{
    const block = await context.chain.getBlock(hash);
    const value = await block?.get(key) || null;
    return value || null;
};
const state_getKeysPaged = async (context, [prefix, pageSize, startKey, hash])=>{
    const block = await context.chain.getBlock(hash);
    return block?.getKeysPaged({
        prefix,
        pageSize,
        startKey
    });
};
const state_queryStorageAt = async (context, [keys, hash])=>{
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
const state_call = async (context, [method, data, hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    const resp = await block.call(method, [
        data
    ]);
    return resp.result;
};
const state_getReadProof = async (context, [keys, hash])=>{
    const { at, proof } = await (0, _readproof.buildReadProof)(context, keys, hash);
    return {
        at,
        proof
    };
};
const state_subscribeRuntimeVersion = async (context, _params, { subscribe })=>{
    let update = (_block)=>{};
    const id = await context.chain.headState.subscribeStorage([
        (0, _util.stringToHex)(':code')
    ], (block)=>update(block));
    const callback = subscribe('state_runtimeVersion', id, ()=>context.chain.headState.unsubscribeStorage(id));
    update = async (block)=>callback(await block.runtimeVersion);
    (async ()=>{
        update(context.chain.head);
    })();
    return id;
};
const state_unsubscribeRuntimeVersion = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
const state_subscribeStorage = async (context, [keys], { subscribe })=>{
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
const state_unsubscribeStorage = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
const childstate_getStorage = async (context, [child, key, hash])=>{
    if (!(0, _index.isPrefixedChildKey)(child)) {
        throw new _shared.ResponseError(-32000, 'Client error: Invalid child storage key');
    }
    const block = await context.chain.getBlock(hash);
    const value = await block?.get((0, _index.prefixedChildKey)(child, key));
    return value || null;
};
const childstate_getKeysPaged = async (context, [child, prefix, pageSize, startKey, hash])=>{
    if (!(0, _index.isPrefixedChildKey)(child)) {
        throw new _shared.ResponseError(-32000, 'Client error: Invalid child storage key');
    }
    const block = await context.chain.getBlock(hash);
    return block?.getKeysPaged({
        prefix: (0, _index.prefixedChildKey)(child, prefix),
        pageSize,
        startKey: (0, _index.prefixedChildKey)(child, startKey)
    }).then((keys)=>keys.map(_index.stripChildPrefix));
};
const state_getStorageAt = state_getStorage;
