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
    get ChainHeadV1RPC () {
        return _chainHead_v1;
    },
    get ChainSpecV1RPC () {
        return _chainSpec_v1;
    },
    get TransactionV1RPC () {
        return _transaction_v1;
    },
    get default () {
        return _default;
    }
});
const _archive_v1 = /*#__PURE__*/ _interop_require_wildcard(require("./archive_v1.js"));
const _chainHead_v1 = /*#__PURE__*/ _interop_require_wildcard(require("./chainHead_v1.js"));
const _chainSpec_v1 = /*#__PURE__*/ _interop_require_wildcard(require("./chainSpec_v1.js"));
const _transaction_v1 = /*#__PURE__*/ _interop_require_wildcard(require("./transaction_v1.js"));
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
const handlers = {
    ..._archive_v1,
    ..._chainHead_v1,
    ..._transaction_v1,
    ..._chainSpec_v1
};
const _default = handlers;
