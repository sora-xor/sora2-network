"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_setBlockBuildMode", {
    enumerable: true,
    get: function() {
        return dev_setBlockBuildMode;
    }
});
const _txpool = require("../../blockchain/txpool.js");
const _logger = require("../../logger.js");
const _shared = require("../shared.js");
const dev_setBlockBuildMode = async (context, [mode])=>{
    _logger.defaultLogger.debug({
        mode: _txpool.BuildBlockMode[mode]
    }, 'dev_setBlockBuildMode');
    if (_txpool.BuildBlockMode[mode] === undefined) {
        throw new _shared.ResponseError(1, `Invalid mode ${mode}`);
    }
    context.chain.txPool.mode = mode;
};
