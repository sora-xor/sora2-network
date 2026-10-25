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
    get transaction_v1_broadcast () {
        return transaction_v1_broadcast;
    },
    get transaction_v1_stop () {
        return transaction_v1_stop;
    }
});
const _logger = require("../../logger.js");
const logger = _logger.defaultLogger.child({
    name: 'rpc-transaction_v1'
});
const randomId = ()=>Math.random().toString(36).substring(2);
const transaction_v1_broadcast = async (context, [extrinsic])=>{
    await context.chain.submitExtrinsic(extrinsic).catch((err)=>{
        // As per the spec, the invalid transaction errors should be ignored.
        logger.warn('Submit extrinsic failed', err);
    });
    return randomId();
};
const transaction_v1_stop = async (_context, [_operationId])=>{
    // Chopsticks doesn't have any process to broadcast the transaction through P2P
    // so stopping doesn't have any effect.
    return null;
};
