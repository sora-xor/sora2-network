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
    get ResponseError () {
        return ResponseError;
    },
    get logger () {
        return logger;
    },
    get zHash () {
        return zHash;
    },
    get zHex () {
        return zHex;
    }
});
const _zod = require("zod");
const _logger = require("../logger.js");
function _define_property(obj, key, value) {
    if (key in obj) {
        Object.defineProperty(obj, key, {
            value: value,
            enumerable: true,
            configurable: true,
            writable: true
        });
    } else {
        obj[key] = value;
    }
    return obj;
}
const logger = _logger.defaultLogger.child({
    name: 'rpc'
});
const zHex = _zod.z.custom((val)=>/^0x\w+$/.test(val));
const zHash = _zod.z.string().length(66).and(zHex);
class ResponseError extends Error {
    toJSON() {
        return {
            code: this.code,
            message: this.message
        };
    }
    constructor(code, message){
        super(message), _define_property(this, "code", void 0);
        this.name = 'ResponseError';
        this.code = code;
    }
}
