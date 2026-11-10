"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_setStorage", {
    enumerable: true,
    get: function() {
        return dev_setStorage;
    }
});
const _logger = require("../../logger.js");
const _setstorage = require("../../utils/set-storage.js");
const _shared = require("../shared.js");
const dev_setStorage = async (context, params)=>{
    const [values, blockHash] = params;
    const hash = await (0, _setstorage.setStorage)(context.chain, values, blockHash).catch((error)=>{
        throw new _shared.ResponseError(1, error.toString());
    });
    _logger.defaultLogger.debug({
        hash,
        values
    }, 'dev_setStorage');
    return hash;
};
