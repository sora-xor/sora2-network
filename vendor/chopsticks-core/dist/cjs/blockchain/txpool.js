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
    get APPLY_EXTRINSIC_ERROR () {
        return APPLY_EXTRINSIC_ERROR;
    },
    get BuildBlockMode () {
        return BuildBlockMode;
    },
    get TxPool () {
        return TxPool;
    }
});
const _toU8a = require("@polkadot/util/hex/toU8a");
const _eventemitter3 = require("eventemitter3");
const _lodash = /*#__PURE__*/ _interop_require_default(require("lodash"));
const _logger = require("../logger.js");
const _index = require("../utils/index.js");
const _blockbuilder = require("./block-builder.js");
function _check_private_redeclaration(obj, privateCollection) {
    if (privateCollection.has(obj)) {
        throw new TypeError("Cannot initialize the same private elements twice on an object");
    }
}
function _class_apply_descriptor_get(receiver, descriptor) {
    if (descriptor.get) {
        return descriptor.get.call(receiver);
    }
    return descriptor.value;
}
function _class_apply_descriptor_set(receiver, descriptor, value) {
    if (descriptor.set) {
        descriptor.set.call(receiver, value);
    } else {
        if (!descriptor.writable) {
            throw new TypeError("attempted to set read only private field");
        }
        descriptor.value = value;
    }
}
function _class_extract_field_descriptor(receiver, privateMap, action) {
    if (!privateMap.has(receiver)) {
        throw new TypeError("attempted to " + action + " private field on non-instance");
    }
    return privateMap.get(receiver);
}
function _class_private_field_get(receiver, privateMap) {
    var descriptor = _class_extract_field_descriptor(receiver, privateMap, "get");
    return _class_apply_descriptor_get(receiver, descriptor);
}
function _class_private_field_init(obj, privateMap, value) {
    _check_private_redeclaration(obj, privateMap);
    privateMap.set(obj, value);
}
function _class_private_field_set(receiver, privateMap, value) {
    var descriptor = _class_extract_field_descriptor(receiver, privateMap, "set");
    _class_apply_descriptor_set(receiver, descriptor, value);
    return value;
}
function _class_private_method_get(receiver, privateSet, fn) {
    if (!privateSet.has(receiver)) {
        throw new TypeError("attempted to get private field on non-instance");
    }
    return fn;
}
function _class_private_method_init(obj, privateSet) {
    _check_private_redeclaration(obj, privateSet);
    privateSet.add(obj);
}
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
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const logger = _logger.defaultLogger.child({
    name: 'txpool'
});
const APPLY_EXTRINSIC_ERROR = 'TxPool::ApplyExtrinsicError';
var BuildBlockMode = /*#__PURE__*/ function(BuildBlockMode) {
    /** One block per batch (default) */ BuildBlockMode["Batch"] = "Batch";
    /** One block per tx */ BuildBlockMode["Instant"] = "Instant";
    /** Only build when triggered */ BuildBlockMode["Manual"] = "Manual";
    return BuildBlockMode;
}({});
var _chain = /*#__PURE__*/ new WeakMap(), _pool = /*#__PURE__*/ new WeakMap(), _ump = /*#__PURE__*/ new WeakMap(), _dmp = /*#__PURE__*/ new WeakMap(), _hrmp = /*#__PURE__*/ new WeakMap(), _mode = /*#__PURE__*/ new WeakMap(), _inherentProviders = /*#__PURE__*/ new WeakMap(), _pendingBlocks = /*#__PURE__*/ new WeakMap(), _isBuilding = /*#__PURE__*/ new WeakMap(), _getSigner = /*#__PURE__*/ new WeakSet(), _maybeBuildBlock = /*#__PURE__*/ new WeakSet(), _batchBuildBlock = /*#__PURE__*/ new WeakMap(), _buildBlockIfNeeded = /*#__PURE__*/ new WeakSet(), _buildBlock = /*#__PURE__*/ new WeakSet();
class TxPool {
    get pendingExtrinsics() {
        return _class_private_field_get(this, _pool).map(({ extrinsic })=>extrinsic);
    }
    get ump() {
        return _class_private_field_get(this, _ump);
    }
    get dmp() {
        return _class_private_field_get(this, _dmp);
    }
    get hrmp() {
        return _class_private_field_get(this, _hrmp);
    }
    get mode() {
        return _class_private_field_get(this, _mode);
    }
    set mode(mode) {
        _class_private_field_set(this, _mode, mode);
    }
    clear() {
        _class_private_field_get(this, _pool).length = 0;
        for (const id of Object.keys(_class_private_field_get(this, _ump))){
            delete _class_private_field_get(this, _ump)[id];
        }
        _class_private_field_get(this, _dmp).length = 0;
        for (const id of Object.keys(_class_private_field_get(this, _hrmp))){
            delete _class_private_field_get(this, _hrmp)[id];
        }
    }
    pendingExtrinsicsBy(address) {
        return _class_private_field_get(this, _pool).filter(({ signer })=>signer === address).map(({ extrinsic })=>extrinsic);
    }
    async submitExtrinsic(extrinsic) {
        logger.debug({
            extrinsic: (0, _logger.truncate)(extrinsic)
        }, 'submit extrinsic');
        _class_private_field_get(this, _pool).push({
            extrinsic,
            signer: await _class_private_method_get(this, _getSigner, getSigner).call(this, extrinsic)
        });
        _class_private_method_get(this, _maybeBuildBlock, maybeBuildBlock).call(this);
    }
    submitUpwardMessages(id, ump) {
        logger.debug({
            id,
            ump: (0, _logger.truncate)(ump)
        }, 'submit upward messages');
        if (!_class_private_field_get(this, _ump)[id]) {
            _class_private_field_get(this, _ump)[id] = [];
        }
        _class_private_field_get(this, _ump)[id].push(...ump);
        _class_private_method_get(this, _maybeBuildBlock, maybeBuildBlock).call(this);
    }
    submitDownwardMessages(dmp) {
        logger.debug({
            dmp: (0, _logger.truncate)(dmp)
        }, 'submit downward messages');
        _class_private_field_get(this, _dmp).push(...dmp);
        _class_private_method_get(this, _maybeBuildBlock, maybeBuildBlock).call(this);
    }
    submitHorizontalMessages(id, hrmp) {
        logger.debug({
            id,
            hrmp: (0, _logger.truncate)(hrmp)
        }, 'submit horizontal messages');
        if (!_class_private_field_get(this, _hrmp)[id]) {
            _class_private_field_get(this, _hrmp)[id] = [];
        }
        _class_private_field_get(this, _hrmp)[id].push(...hrmp);
        _class_private_method_get(this, _maybeBuildBlock, maybeBuildBlock).call(this);
    }
    async buildBlockWithParams(params) {
        _class_private_field_get(this, _pendingBlocks).push({
            params,
            deferred: (0, _index.defer)()
        });
        _class_private_method_get(this, _buildBlockIfNeeded, buildBlockIfNeeded).call(this);
        await this.upcomingBlocks();
    }
    async buildBlock(params) {
        const transactions = params?.transactions || _class_private_field_get(this, _pool).splice(0).map(({ extrinsic })=>extrinsic);
        const upwardMessages = params?.upwardMessages || {
            ..._class_private_field_get(this, _ump)
        };
        const downwardMessages = params?.downwardMessages || _class_private_field_get(this, _dmp).splice(0);
        const horizontalMessages = params?.horizontalMessages || {
            ..._class_private_field_get(this, _hrmp)
        };
        const unsafeBlockHeight = params?.unsafeBlockHeight;
        const relayChainStateOverrides = params?.relayChainStateOverrides;
        const relayParentNumber = params?.relayParentNumber;
        if (!params?.upwardMessages) {
            for (const id of Object.keys(_class_private_field_get(this, _ump))){
                delete _class_private_field_get(this, _ump)[id];
            }
        }
        if (!params?.horizontalMessages) {
            for (const id of Object.keys(_class_private_field_get(this, _hrmp))){
                delete _class_private_field_get(this, _hrmp)[id];
            }
        }
        try {
            await this.buildBlockWithParams({
                transactions,
                upwardMessages,
                downwardMessages,
                horizontalMessages,
                unsafeBlockHeight,
                relayChainStateOverrides,
                relayParentNumber
            });
            // with the latest message queue, messages could be processed in the upcoming block
            if (!_class_private_field_get(this, _chain).processQueuedMessages) return;
            // if block was built without horizontal or downward messages then skip
            if (_lodash.default.isEmpty(horizontalMessages) && _lodash.default.isEmpty(downwardMessages)) return;
            // messageQueue.bookStateFor
            const prefix = '0xb8753e9383841da95f7b8871e5de326954e062a2cf8df68178ee2e5dbdf00bff';
            const meta = await _class_private_field_get(this, _chain).head.meta;
            const keys = await _class_private_field_get(this, _chain).head.getKeysPaged({
                prefix,
                pageSize: 1000
            });
            for (const key of keys){
                const rawValue = await _class_private_field_get(this, _chain).head.get(key);
                if (!rawValue) continue;
                const message = meta.registry.createType('PalletMessageQueueBookState', (0, _toU8a.hexToU8a)(rawValue)).toJSON();
                if (message.size > 0 && message.end > message.begin) {
                    logger.info('Queued messages detected, building a new block');
                    // build a new block to process the queued messages
                    await _class_private_field_get(this, _chain).newBlock();
                    return;
                }
            }
        } catch (err) {
            logger.error({
                err
            }, 'build block failed');
        }
    }
    async upcomingBlocks() {
        const count = _class_private_field_get(this, _pendingBlocks).length;
        if (count > 0) {
            await _class_private_field_get(this, _pendingBlocks)[count - 1].deferred.promise;
        }
        return count;
    }
    constructor(chain, inherentProviders, mode = "Batch"){
        _class_private_method_init(this, _getSigner);
        _class_private_method_init(this, _maybeBuildBlock);
        _class_private_method_init(this, _buildBlockIfNeeded);
        _class_private_method_init(this, _buildBlock);
        _class_private_field_init(this, _chain, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _pool, {
            writable: true,
            value: []
        });
        _class_private_field_init(this, _ump, {
            writable: true,
            value: {}
        });
        _class_private_field_init(this, _dmp, {
            writable: true,
            value: []
        });
        _class_private_field_init(this, _hrmp, {
            writable: true,
            value: {}
        });
        _class_private_field_init(this, _mode, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _inherentProviders, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _pendingBlocks, {
            writable: true,
            value: []
        });
        _define_property(this, "event", new _eventemitter3.EventEmitter());
        _class_private_field_init(this, _isBuilding, {
            writable: true,
            value: false
        });
        _class_private_field_init(this, _batchBuildBlock, {
            writable: true,
            value: _lodash.default.debounce(this.buildBlock, 100, {
                maxWait: 1000
            })
        });
        _class_private_field_set(this, _chain, chain);
        _class_private_field_set(this, _mode, mode);
        _class_private_field_set(this, _inherentProviders, inherentProviders);
    }
}
async function getSigner(extrinsic) {
    const registry = await _class_private_field_get(this, _chain).head.registry;
    const tx = registry.createType('GenericExtrinsic', extrinsic);
    return tx.signer.toString();
}
function maybeBuildBlock() {
    switch(_class_private_field_get(this, _mode)){
        case "Batch":
            _class_private_field_get(this, _batchBuildBlock).call(this);
            break;
        case "Instant":
            this.buildBlock();
            break;
        case "Manual":
            break;
    }
}
async function buildBlockIfNeeded() {
    if (_class_private_field_get(this, _isBuilding)) return;
    if (_class_private_field_get(this, _pendingBlocks).length === 0) return;
    _class_private_field_set(this, _isBuilding, true);
    try {
        await _class_private_method_get(this, _buildBlock, buildBlock).call(this);
    } catch (error) {
        logger.error({
            error
        }, 'build block failed');
        for (const { deferred } of _class_private_field_get(this, _pendingBlocks)){
            deferred.reject(error);
        }
        _class_private_field_get(this, _pendingBlocks).length = 0;
    } finally{
        _class_private_field_set(this, _isBuilding, false);
    }
    _class_private_method_get(this, _buildBlockIfNeeded, buildBlockIfNeeded).call(this);
}
async function buildBlock() {
    await _class_private_field_get(this, _chain).api.isReady;
    const pending = _class_private_field_get(this, _pendingBlocks)[0];
    if (!pending) {
        throw new Error('Unreachable');
    }
    const { params, deferred } = pending;
    logger.trace({
        params
    }, 'build block');
    const [newBlock, pendingExtrinsics] = await (0, _blockbuilder.buildBlock)(_class_private_field_get(this, _chain).head, _class_private_field_get(this, _inherentProviders), params, {
        onApplyExtrinsicError: (extrinsic, error)=>{
            this.event.emit(APPLY_EXTRINSIC_ERROR, [
                extrinsic,
                error
            ]);
        },
        onPhaseApplied: logger.level.toLowerCase() === 'trace' ? (phase, resp)=>{
            switch(phase){
                case 'initialize':
                    logger.trace((0, _logger.truncate)(resp.storageDiff), 'Initialize block');
                    break;
                case 'finalize':
                    logger.trace((0, _logger.truncate)(resp.storageDiff), 'Finalize block');
                    break;
                default:
                    logger.trace((0, _logger.truncate)(resp.storageDiff), `Apply extrinsic ${phase}`);
            }
        } : undefined
    });
    for (const extrinsic of pendingExtrinsics){
        _class_private_field_get(this, _pool).push({
            extrinsic,
            signer: await _class_private_method_get(this, _getSigner, getSigner).call(this, extrinsic)
        });
    }
    await _class_private_field_get(this, _chain).onNewBlock(newBlock);
    _class_private_field_get(this, _pendingBlocks).shift();
    deferred.resolve();
}
