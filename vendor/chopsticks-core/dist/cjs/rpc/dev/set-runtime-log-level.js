"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_setRuntimeLogLevel", {
    enumerable: true,
    get: function() {
        return dev_setRuntimeLogLevel;
    }
});
const _logger = require("../../logger.js");
const _shared = require("../shared.js");
const dev_setRuntimeLogLevel = async (context, [runtimeLogLevel])=>{
    _logger.defaultLogger.debug({
        runtimeLogLevel
    }, 'dev_setRuntimeLogLevel');
    if (typeof runtimeLogLevel !== 'number') {
        throw new _shared.ResponseError(1, `Invalid runtimeLogLevel ${runtimeLogLevel}`);
    }
    context.chain.runtimeLogLevel = runtimeLogLevel;
};
