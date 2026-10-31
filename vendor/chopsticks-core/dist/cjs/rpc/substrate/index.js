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
    get ArchiveRPC () {
        return _archive;
    },
    get AuthorRPC () {
        return _author;
    },
    get ChainRPC () {
        return _chain;
    },
    get PaymentRPC () {
        return _payment;
    },
    get StateRPC () {
        return _state;
    },
    get SystemRPC () {
        return _system;
    },
    get default () {
        return _default;
    }
});
const _archive = /*#__PURE__*/ _interop_require_wildcard(require("./archive.js"));
const _author = /*#__PURE__*/ _interop_require_wildcard(require("./author.js"));
const _chain = /*#__PURE__*/ _interop_require_wildcard(require("./chain.js"));
const _payment = /*#__PURE__*/ _interop_require_wildcard(require("./payment.js"));
const _state = /*#__PURE__*/ _interop_require_wildcard(require("./state.js"));
const _system = /*#__PURE__*/ _interop_require_wildcard(require("./system.js"));
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
    ..._archive,
    ..._author,
    ..._chain,
    ..._payment,
    ..._state,
    ..._system
};
const _default = handlers;
