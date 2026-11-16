"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_newBlock", {
    enumerable: true,
    get: function() {
        return dev_newBlock;
    }
});
const _zod = require("zod");
const _logger = require("../../logger.js");
const _shared = require("../shared.js");
const schema = _zod.z.object({
    count: _zod.z.number().optional(),
    to: _zod.z.number().optional(),
    dmp: _zod.z.array(_zod.z.object({
        sentAt: _zod.z.number(),
        msg: _shared.zHex
    })).min(1).optional(),
    ump: _zod.z.record(_zod.z.number(), _zod.z.array(_shared.zHex).min(1)).optional(),
    hrmp: _zod.z.record(_zod.z.union([
        _zod.z.number(),
        _zod.z.string()
    ]), _zod.z.array(_zod.z.object({
        sentAt: _zod.z.number(),
        data: _shared.zHex
    })).min(1)).optional(),
    transactions: _zod.z.array(_shared.zHex).min(1).optional(),
    unsafeBlockHeight: _zod.z.number().optional(),
    relayChainStateOverrides: _zod.z.array(_zod.z.tuple([
        _shared.zHex,
        _zod.z.union([
            _shared.zHex,
            _zod.z.null()
        ])
    ])).optional(),
    relayParentNumber: _zod.z.number().optional()
});
const dev_newBlock = async (context, [params])=>{
    const { count, to, hrmp, ump, dmp, transactions, unsafeBlockHeight, relayChainStateOverrides, relayParentNumber } = schema.parse(params || {});
    const now = context.chain.head.number;
    const diff = to ? to - now : count;
    const finalCount = diff !== undefined ? Math.max(diff, 1) : 1;
    let finalHash;
    if (unsafeBlockHeight !== undefined && unsafeBlockHeight <= now) {
        throw new _shared.ResponseError(1, 'unsafeBlockHeight must be greater than current block height');
    }
    for(let i = 0; i < finalCount; i++){
        const block = await context.chain.newBlock({
            transactions,
            horizontalMessages: hrmp,
            upwardMessages: ump,
            downwardMessages: dmp,
            unsafeBlockHeight: i === 0 ? unsafeBlockHeight : undefined,
            relayChainStateOverrides: relayChainStateOverrides,
            relayParentNumber: relayParentNumber
        }).catch((error)=>{
            throw new _shared.ResponseError(1, error.toString());
        });
        _logger.defaultLogger.debug({
            hash: block.hash
        }, 'dev_newBlock');
        finalHash = block.hash;
    }
    return finalHash;
};
