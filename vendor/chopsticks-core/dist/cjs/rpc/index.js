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
        return _shared.ResponseError;
    },
    get allHandlers () {
        return allHandlers;
    },
    get dev () {
        return _index.default;
    },
    get substrate () {
        return _index2.default;
    }
});
const _index = /*#__PURE__*/ _interop_require_default(require("./dev/index.js"));
const _index1 = /*#__PURE__*/ _interop_require_default(require("./rpc-spec/index.js"));
const _index2 = /*#__PURE__*/ _interop_require_default(require("./substrate/index.js"));
const _shared = require("./shared.js");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const allHandlers = {
    ..._index2.default,
    ..._index1.default,
    ..._index.default,
    rpc_methods: async ()=>Promise.resolve({
            version: 1,
            methods: Object.keys(allHandlers).sort()
        })
};
