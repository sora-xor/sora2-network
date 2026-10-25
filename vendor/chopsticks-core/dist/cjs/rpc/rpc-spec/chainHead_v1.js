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
    get chainHead_v1_body () {
        return chainHead_v1_body;
    },
    get chainHead_v1_call () {
        return chainHead_v1_call;
    },
    get chainHead_v1_continue () {
        return chainHead_v1_continue;
    },
    get chainHead_v1_follow () {
        return chainHead_v1_follow;
    },
    get chainHead_v1_header () {
        return chainHead_v1_header;
    },
    get chainHead_v1_stopOperation () {
        return chainHead_v1_stopOperation;
    },
    get chainHead_v1_storage () {
        return chainHead_v1_storage;
    },
    get chainHead_v1_unfollow () {
        return chainHead_v1_unfollow;
    },
    get chainHead_v1_unpin () {
        return chainHead_v1_unpin;
    }
});
const _utilcrypto = require("@polkadot/util-crypto");
const _logger = require("../../logger.js");
const _shared = require("../shared.js");
const _storagecommon = require("./storage-common.js");
const logger = _logger.defaultLogger.child({
    name: 'rpc-chainHead_v1'
});
const following = new Map();
const chainHead_v1_follow = async (context, [withRuntime], { subscribe })=>{
    const update = async (block)=>{
        logger.trace({
            hash: block.hash
        }, 'chainHead_v1_follow');
        const getNewRuntime = async ()=>{
            const [runtime, previousRuntime] = await Promise.all([
                block.runtimeVersion,
                block.parentBlock.then((b)=>b?.runtimeVersion)
            ]);
            const hasNewRuntime = runtime.implVersion !== previousRuntime?.implVersion || runtime.specVersion !== previousRuntime.specVersion;
            return hasNewRuntime ? runtime : null;
        };
        const newRuntime = withRuntime ? await getNewRuntime() : null;
        callback({
            event: 'newBlock',
            blockHash: block.hash,
            parentBlockHash: (await block.parentBlock)?.hash,
            newRuntime
        });
        callback({
            event: 'bestBlockChanged',
            bestBlockHash: block.hash
        });
        callback({
            event: 'finalized',
            finalizedBlockHashes: [
                block.hash
            ],
            prunedBlockHashes: []
        });
        const storageDiffs = following.get(id)?.storageDiffs;
        if (storageDiffs?.size) {
            // Fetch the storage diffs and update the `closestDescendantMerkleValue` for those that changed
            const diffKeys = Object.keys(await block.storageDiff());
            for (const [prefix, value] of storageDiffs.entries()){
                if (diffKeys.some((key)=>key.startsWith(prefix))) {
                    storageDiffs.set(prefix, value + 1);
                }
            }
        }
    };
    const id = context.chain.headState.subscribeHead(update);
    const cleanup = ()=>{
        context.chain.headState.unsubscribeHead(id);
        following.delete(id);
    };
    const callback = subscribe('chainHead_v1_followEvent', id, cleanup);
    following.set(id, {
        callback,
        pendingDescendantValues: new Map(),
        storageDiffs: new Map()
    });
    (0, _storagecommon.afterResponse)(async ()=>{
        callback({
            event: 'initialized',
            finalizedBlockHashes: [
                context.chain.head.hash
            ],
            finalizedBlockRuntime: withRuntime ? await context.chain.head.runtimeVersion : null
        });
        callback({
            event: 'bestBlockChanged',
            bestBlockHash: context.chain.head.hash
        });
    });
    return id;
};
const chainHead_v1_unfollow = async (_, [followSubscription], { unsubscribe })=>{
    unsubscribe(followSubscription);
    return null;
};
const chainHead_v1_header = async (context, [followSubscription, hash])=>{
    if (!following.has(followSubscription)) return null;
    const block = await context.chain.getBlock(hash);
    return block ? (await block.header).toHex() : null;
};
const operationStarted = (operationId)=>({
        result: 'started',
        operationId
    });
const randomId = ()=>Math.random().toString(36).substring(2);
const chainHead_v1_call = async (context, [followSubscription, hash, method, callParameters])=>{
    const operationId = randomId();
    (0, _storagecommon.afterResponse)(async ()=>{
        const block = await context.chain.getBlock(hash);
        if (!block) {
            following.get(followSubscription)?.callback({
                event: 'operationError',
                operationId,
                error: `Block ${hash} not found`
            });
        } else {
            try {
                const resp = await block.call(method, [
                    callParameters
                ]);
                following.get(followSubscription)?.callback({
                    event: 'operationCallDone',
                    operationId,
                    output: resp.result
                });
            } catch (ex) {
                following.get(followSubscription)?.callback({
                    event: 'operationError',
                    operationId,
                    error: ex.message
                });
            }
        }
    });
    return operationStarted(operationId);
};
const chainHead_v1_storage = async (context, [followSubscription, hash, items, _childTrie])=>{
    const operationId = randomId();
    (0, _storagecommon.afterResponse)(async ()=>{
        const block = await context.chain.getBlock(hash);
        if (!block) {
            following.get(followSubscription)?.callback({
                event: 'operationError',
                operationId,
                error: 'Block not found'
            });
            return;
        }
        const handleStorageItemRequest = async (sir)=>{
            switch(sir.type){
                case 'value':
                    {
                        const value = await block.get(sir.key);
                        if (value) {
                            following.get(followSubscription)?.callback({
                                event: 'operationStorageItems',
                                operationId,
                                items: [
                                    {
                                        key: sir.key,
                                        value
                                    }
                                ]
                            });
                        }
                        return null;
                    }
                case 'hash':
                    {
                        const value = await block.get(sir.key);
                        if (value) {
                            following.get(followSubscription)?.callback({
                                event: 'operationStorageItems',
                                operationId,
                                items: [
                                    {
                                        key: sir.key,
                                        hash: (0, _utilcrypto.blake2AsHex)(value)
                                    }
                                ]
                            });
                        }
                        return null;
                    }
                case 'descendantsValues':
                    {
                        const { items, next } = await (0, _storagecommon.getDescendantValues)(block, {
                            prefix: sir.key,
                            startKey: '0x'
                        });
                        following.get(followSubscription)?.callback({
                            event: 'operationStorageItems',
                            operationId,
                            items
                        });
                        return next;
                    }
                case 'descendantsHashes':
                    {
                        const { items, next } = await (0, _storagecommon.getDescendantValues)(block, {
                            prefix: sir.key,
                            startKey: '0x'
                        });
                        following.get(followSubscription)?.callback({
                            event: 'operationStorageItems',
                            operationId,
                            items: items.map(({ key, value })=>({
                                    key,
                                    hash: value !== undefined ? (0, _utilcrypto.blake2AsHex)(value) : undefined
                                }))
                        });
                        return next ? {
                            ...next,
                            isDescendantHashes: true
                        } : null;
                    }
                case 'closestDescendantMerkleValue':
                    {
                        const followingSubscription = following.get(followSubscription);
                        if (!followingSubscription) return null;
                        if (!followingSubscription.storageDiffs.has(sir.key)) {
                            // Set up a diff watch for this key
                            followingSubscription.storageDiffs.set(sir.key, 0);
                        }
                        followingSubscription.callback({
                            event: 'operationStorageItems',
                            operationId,
                            items: [
                                {
                                    key: sir.key,
                                    closestDescendantMerkleValue: String(followingSubscription.storageDiffs.get(sir.key))
                                }
                            ]
                        });
                        return null;
                    }
            }
        };
        const listResult = await Promise.all(items.map(handleStorageItemRequest));
        const pending = listResult.filter((v)=>v !== null);
        if (!pending.length) {
            following.get(followSubscription)?.callback({
                event: 'operationStorageDone',
                operationId
            });
        } else {
            const follower = following.get(followSubscription);
            if (follower) {
                follower.pendingDescendantValues.set(operationId, {
                    hash,
                    params: pending
                });
                follower.callback({
                    event: 'operationWaitingForContinue',
                    operationId
                });
            }
        }
    });
    return {
        ...operationStarted(operationId),
        discardedItems: 0
    };
};
const limitReached = {
    result: 'limitReached'
};
const chainHead_v1_body = async (context, [followSubscription, hash])=>{
    if (!following.has(followSubscription)) return limitReached;
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(-32801, 'Block not found');
    }
    const operationId = randomId();
    (0, _storagecommon.afterResponse)(async ()=>{
        const body = await block.extrinsics;
        following.get(followSubscription)?.callback({
            event: 'operationBodyDone',
            operationId,
            value: body
        });
    });
    return operationStarted(operationId);
};
const chainHead_v1_continue = async (context, [followSubscription, operationId])=>{
    const follower = following.get(followSubscription);
    const pendingOp = follower?.pendingDescendantValues.get(operationId);
    if (!pendingOp || !follower) {
        throw new _shared.ResponseError(-32803, "Operation ID doesn't have anything pending");
    }
    const block = await context.chain.getBlock(pendingOp.hash);
    if (!block) {
        throw new _shared.ResponseError(-32801, 'Block not found');
    }
    (0, _storagecommon.afterResponse)(async ()=>{
        const handlePendingOperation = async (params)=>{
            const { items, next } = await (0, _storagecommon.getDescendantValues)(block, params);
            follower.callback({
                event: 'operationStorageItems',
                operationId,
                items: params.isDescendantHashes ? items.map(({ key, value })=>({
                        key,
                        hash: value !== undefined ? (0, _utilcrypto.blake2AsHex)(value) : value
                    })) : items
            });
            return next;
        };
        const listResult = await Promise.all(pendingOp.params.map(handlePendingOperation));
        const pending = listResult.filter((v)=>v !== null);
        if (!pending.length) {
            follower.pendingDescendantValues.delete(operationId);
            follower.callback({
                event: 'operationStorageDone',
                operationId
            });
        } else {
            follower.pendingDescendantValues.set(operationId, {
                hash: pendingOp.hash,
                params: pending
            });
            follower.callback({
                event: 'operationWaitingForContinue',
                operationId
            });
        }
    });
    return null;
};
const chainHead_v1_stopOperation = async (_context, [followSubscription, operationId])=>{
    following.get(followSubscription)?.pendingDescendantValues.delete(operationId);
    return null;
};
const chainHead_v1_unpin = async (_context, [_followSubscription, _hashOrHashes])=>{
    return null;
};
