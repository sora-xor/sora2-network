"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
const _chopsticksexecutor = /*#__PURE__*/ _interop_require_wildcard(require("@acala-network/chopsticks-executor"));
const _comlink = /*#__PURE__*/ _interop_require_wildcard(require("comlink"));
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
const getRuntimeVersion = async (code)=>{
    return _chopsticksexecutor.get_runtime_version(code);
};
// trie_version: 0 for old trie, 1 for new trie
const calculateStateRoot = async (entries, trie_version)=>{
    return _chopsticksexecutor.calculate_state_root(entries, trie_version);
};
const decodeProof = async (trieRootHash, nodes)=>{
    return _chopsticksexecutor.decode_proof(trieRootHash, nodes);
};
const createProof = async (nodes, updates)=>{
    return _chopsticksexecutor.create_proof(nodes, updates);
};
const runTask = async (task, callback)=>{
    return _chopsticksexecutor.run_task(task, callback);
};
const testing = async (callback, key)=>{
    return _chopsticksexecutor.testing(callback, key);
};
const wasmExecutor = {
    runTask,
    getRuntimeVersion,
    calculateStateRoot,
    createProof,
    decodeProof,
    testing
};
_comlink.expose(wasmExecutor);
