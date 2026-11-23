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
    get payment_queryFeeDetails () {
        return payment_queryFeeDetails;
    },
    get payment_queryInfo () {
        return payment_queryInfo;
    }
});
const _util = require("@polkadot/util");
const _shared = require("../shared.js");
const payment_queryFeeDetails = async (context, [extrinsic, hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    const registry = await block.registry;
    const tx = (0, _util.hexToU8a)(extrinsic);
    const resp = await block.call('TransactionPaymentApi_query_fee_details', [
        registry.createType('Extrinsic', tx).toHex(),
        registry.createType('u32', tx.byteLength).toHex()
    ]);
    return resp.result;
};
const payment_queryInfo = async (context, [extrinsic, hash])=>{
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash} not found`);
    }
    const registry = await block.registry;
    const tx = (0, _util.hexToU8a)(extrinsic);
    const resp = await block.call('TransactionPaymentApi_query_info', [
        registry.createType('Extrinsic', tx).toHex(),
        registry.createType('u32', tx.byteLength).toHex()
    ]);
    return resp.result;
};
