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
    get calculateStateRoot () {
        return calculateStateRoot;
    },
    get createProof () {
        return createProof;
    },
    get decodeProof () {
        return decodeProof;
    },
    get destroyWorker () {
        return destroyWorker;
    },
    get emptyTaskHandler () {
        return emptyTaskHandler;
    },
    get getAuraSlotDuration () {
        return getAuraSlotDuration;
    },
    get getRuntimeVersion () {
        return getRuntimeVersion;
    },
    get getWorker () {
        return getWorker;
    },
    get runTask () {
        return runTask;
    },
    get taskHandler () {
        return taskHandler;
    }
});
const _util = require("@polkadot/util");
const _utilcrypto = require("@polkadot/util-crypto");
const _comlink = /*#__PURE__*/ _interop_require_wildcard(require("comlink"));
const _lodash = /*#__PURE__*/ _interop_require_default(require("lodash"));
const _logger = require("../logger.js");
const _index = require("../utils/index.js");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
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
const logger = _logger.defaultLogger.child({
    name: 'executor'
});
let __executor_worker;
const getWorker = async ()=>{
    if (__executor_worker) return __executor_worker;
    const isNode = typeof process !== 'undefined' && process?.versions?.node // true for node or bun
    ;
    if (isNode) {
        __executor_worker = Promise.resolve().then(()=>/*#__PURE__*/ _interop_require_wildcard(require("./node-worker.js"))).then(({ startWorker })=>startWorker());
    } else {
        __executor_worker = Promise.resolve().then(()=>/*#__PURE__*/ _interop_require_wildcard(require("./browser-worker.js"))).then(({ startWorker })=>startWorker());
    }
    return __executor_worker;
};
const getRuntimeVersion = _lodash.default.memoize(async (code)=>{
    const worker = await getWorker();
    return worker.remote.getRuntimeVersion(code).then((version)=>{
        version.specName = (0, _util.hexToString)(version.specName);
        version.implName = (0, _util.hexToString)(version.implName);
        return version;
    });
});
const calculateStateRoot = async (entries, trie_version)=>{
    const worker = await getWorker();
    return worker.remote.calculateStateRoot(entries, trie_version);
};
const decodeProof = async (trieRootHash, nodes)=>{
    const worker = await getWorker();
    const result = await worker.remote.decodeProof(trieRootHash, nodes);
    return result.reduce((accum, [key, value])=>{
        accum[key] = value;
        return accum;
    }, {});
};
const createProof = async (nodes, updates)=>{
    const worker = await getWorker();
    const [trieRootHash, newNodes] = await worker.remote.createProof(nodes, updates);
    return {
        trieRootHash,
        nodes: newNodes
    };
};
let nextTaskId = 0;
const runTask = async (task, callback = emptyTaskHandler, overrideMockSignatureHost = false)=>{
    const taskId = nextTaskId++;
    const task2 = {
        ...task,
        id: taskId,
        storageProofSize: task.storageProofSize ?? 0,
        mockSignatureHost: overrideMockSignatureHost ? 2 : task.mockSignatureHost ? 1 : 0
    };
    const worker = await getWorker();
    logger.trace((0, _logger.truncate)(task2), `runTask #${taskId}`);
    const response = await worker.remote.runTask(task2, _comlink.proxy(callback));
    if ('Call' in response) {
        logger.trace((0, _logger.truncate)(response.Call), `taskResponse #${taskId}`);
    } else {
        logger.trace({
            response
        }, `taskResponse ${taskId}`);
    }
    return response;
};
const taskHandler = (block)=>{
    return {
        getStorage: async (key)=>block.get(key),
        getNextKey: async (prefix, key)=>{
            const [nextKey] = await block.getKeysPaged({
                prefix: prefix.length === 2 /** 0x */  ? key.slice(0, _index.PREFIX_LENGTH) : prefix,
                pageSize: 1,
                startKey: key
            });
            return nextKey && (0, _index.stripChildPrefix)(nextKey);
        },
        offchainGetStorage: async (key)=>{
            if (!block.chain.offchainWorker) throw new Error('offchain worker not found');
            return block.chain.offchainWorker.get(key);
        },
        offchainTimestamp: async ()=>Date.now(),
        offchainRandomSeed: async ()=>(0, _utilcrypto.randomAsHex)(32),
        offchainSubmitTransaction: async (tx)=>{
            if (!block.chain.offchainWorker) throw new Error('offchain worker not found');
            try {
                const hash = await block.chain.offchainWorker.pushExtrinsic(block, tx);
                logger.trace({
                    hash
                }, 'offchainSubmitTransaction');
                return true;
            } catch (error) {
                logger.trace({
                    error
                }, 'offchainSubmitTransaction');
                return false;
            }
        }
    };
};
const emptyTaskHandler = {
    getStorage: async (_key)=>{
        throw new Error('Method not implemented');
    },
    getNextKey: async (_prefix, _key)=>{
        throw new Error('Method not implemented');
    },
    offchainGetStorage: async (_key)=>{
        throw new Error('Method not implemented');
    },
    offchainTimestamp: async ()=>{
        throw new Error('Method not implemented');
    },
    offchainRandomSeed: async ()=>{
        throw new Error('Method not implemented');
    },
    offchainSubmitTransaction: async (_tx)=>{
        throw new Error('Method not implemented');
    }
};
const getAuraSlotDuration = _lodash.default.memoize(async (wasm)=>{
    const result = await runTask({
        wasm,
        calls: [
            [
                'AuraApi_slot_duration',
                []
            ]
        ],
        mockSignatureHost: false,
        allowUnresolvedImports: false,
        runtimeLogLevel: 0
    });
    if ('Error' in result) throw new Error(result.Error);
    return (0, _util.u8aToBn)((0, _util.hexToU8a)(result.Call.result).subarray(0, 8 /* u64: 8 bytes */ )).toNumber();
});
const destroyWorker = async ()=>{
    if (!__executor_worker) return;
    const executor = await __executor_worker;
    executor.remote[_comlink.releaseProxy]();
    await new Promise((resolve)=>setTimeout(resolve, 50));
    await executor.terminate();
    __executor_worker = undefined;
};
