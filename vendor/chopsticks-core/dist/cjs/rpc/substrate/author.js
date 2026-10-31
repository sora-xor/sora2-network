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
    get author_pendingExtrinsics () {
        return author_pendingExtrinsics;
    },
    get author_submitAndWatchExtrinsic () {
        return author_submitAndWatchExtrinsic;
    },
    get author_submitExtrinsic () {
        return author_submitExtrinsic;
    },
    get author_unwatchExtrinsic () {
        return author_unwatchExtrinsic;
    }
});
const _txpool = require("../../blockchain/txpool.js");
const _logger = require("../../logger.js");
const _shared = require("../shared.js");
const logger = _logger.defaultLogger.child({
    name: 'rpc-author'
});
const author_submitExtrinsic = async (context, [extrinsic])=>{
    return context.chain.submitExtrinsic(extrinsic).catch((error)=>{
        const code = error.isInvalid ? 1010 : 1011;
        throw new _shared.ResponseError(code, error.toString());
    });
};
const author_submitAndWatchExtrinsic = async (context, [extrinsic], { subscribe, unsubscribe })=>{
    let update = (_block)=>{};
    const id = context.chain.headState.subscribeHead((block)=>update(block));
    const callback = subscribe('author_extrinsicUpdate', id, ()=>context.chain.headState.unsubscribeHead(id));
    const onExtrinsicFail = ([failedExtrinsic, error])=>{
        if (failedExtrinsic === extrinsic) {
            callback(error.toJSON());
            done(id);
        }
    };
    context.chain.txPool.event.on(_txpool.APPLY_EXTRINSIC_ERROR, onExtrinsicFail);
    const done = (id)=>{
        context.chain.txPool.event.removeListener(_txpool.APPLY_EXTRINSIC_ERROR, onExtrinsicFail);
        unsubscribe(id);
    };
    update = async (block)=>{
        const extrisnics = await block.extrinsics;
        if (!extrisnics.includes(extrinsic)) return;
        logger.debug({
            block: block.hash
        }, 'author_extrinsicUpdate');
        callback({
            inBlock: block.hash
        });
        // wait a bit for InBlock to be sent
        await new Promise((r)=>setTimeout(r, 100));
        callback({
            finalized: block.hash
        });
        done(id);
    };
    try {
        await context.chain.submitExtrinsic(extrinsic);
        // send callback after subscription id is returned
        setTimeout(()=>{
            callback({
                ready: null
            });
            // fake broadcast to alice peer
            callback({
                broadcast: [
                    '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
                ]
            });
        }, 50);
    } catch (error) {
        logger.error({
            error
        }, 'ExtrinsicFailed');
        const code = error.isInvalid ? 1010 : 1011;
        done(id);
        throw new _shared.ResponseError(code, error.toString());
    }
    return id;
};
const author_unwatchExtrinsic = async (_context, [subid], { unsubscribe })=>{
    unsubscribe(subid);
};
const author_pendingExtrinsics = async (context)=>{
    return context.chain.txPool.pendingExtrinsics;
};
