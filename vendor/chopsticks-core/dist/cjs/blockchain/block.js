"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "Block", {
    enumerable: true,
    get: function() {
        return Block;
    }
});
const _types = require("@polkadot/types");
const _metadata = require("@polkadot/types/metadata");
const _util = require("@polkadot/util");
const _index = require("../utils/index.js");
const _index1 = require("../wasm-executor/index.js");
const _storagelayer = require("./storage-layer.js");
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
var _chain = /*#__PURE__*/ new WeakMap(), _header = /*#__PURE__*/ new WeakMap(), _parentBlock = /*#__PURE__*/ new WeakMap(), _extrinsics = /*#__PURE__*/ new WeakMap(), _wasm = /*#__PURE__*/ new WeakMap(), _runtimeVersion = /*#__PURE__*/ new WeakMap(), _metadata1 = /*#__PURE__*/ new WeakMap(), _registry = /*#__PURE__*/ new WeakMap(), _meta = /*#__PURE__*/ new WeakMap(), _baseStorage = /*#__PURE__*/ new WeakMap(), _storages = /*#__PURE__*/ new WeakMap();
class Block {
    get chain() {
        return _class_private_field_get(this, _chain);
    }
    get header() {
        if (!_class_private_field_get(this, _header)) {
            _class_private_field_set(this, _header, Promise.all([
                this.registry,
                _class_private_field_get(this, _chain).api.getHeader(this.hash)
            ]).then(([registry, header])=>registry.createType('Header', header)));
        }
        return _class_private_field_get(this, _header);
    }
    get extrinsics() {
        if (!_class_private_field_get(this, _extrinsics)) {
            _class_private_field_set(this, _extrinsics, _class_private_field_get(this, _chain).api.getBlock(this.hash).then((b)=>{
                if (!b) {
                    throw new Error(`Block ${this.hash} not found`);
                }
                return b.block.extrinsics;
            }));
        }
        return _class_private_field_get(this, _extrinsics);
    }
    get parentBlock() {
        if (this.number === 0) {
            return Promise.resolve(undefined);
        }
        const getBlock = async (header)=>{
            const _header = await header;
            const block = await _class_private_field_get(this, _chain).getBlock(_header.parentHash.toHex());
            if (block) _class_private_field_set(this, _parentBlock, new WeakRef(block));
            return block;
        };
        if (_class_private_field_get(this, _parentBlock) instanceof WeakRef) {
            const block = _class_private_field_get(this, _parentBlock).deref();
            if (block) return Promise.resolve(block);
            _class_private_field_set(this, _parentBlock, getBlock(this.header));
        } else if (!_class_private_field_get(this, _parentBlock)) {
            _class_private_field_set(this, _parentBlock, getBlock(this.header));
        }
        return _class_private_field_get(this, _parentBlock);
    }
    /**
   * Get the block storage.
   */ get storage() {
        return _class_private_field_get(this, _storages)[_class_private_field_get(this, _storages).length - 1] ?? _class_private_field_get(this, _baseStorage);
    }
    /**
   * Get the block storage by key.
   */ async get(key) {
        const val = await this.storage.get(key, true);
        switch(val){
            case _storagelayer.StorageValueKind.Deleted:
                return undefined;
            default:
                return val;
        }
    }
    /**
   * Get the block storage by key.
   */ async getMany(keys) {
        const vals = await this.storage.getMany(keys, true);
        return vals.map((val)=>{
            switch(val){
                case _storagelayer.StorageValueKind.Deleted:
                    return undefined;
                default:
                    return val;
            }
        });
    }
    async read(type, query, ...args) {
        const key = (0, _index.compactHex)(query(...args));
        const value = await this.get(key);
        if (!value) {
            return undefined;
        }
        const registry = await this.registry;
        return registry.createType(type, (0, _util.hexToU8a)(value));
    }
    /**
   * Get paged storage keys.
   */ async getKeysPaged(options) {
        const prefix = options.prefix ?? '0x';
        const startKey = options.startKey ?? '0x';
        const pageSize = options.pageSize;
        return this.storage.getKeysPaged(prefix, pageSize, startKey);
    }
    /**
   * Push a layer to the storage stack.
   */ pushStorageLayer() {
        const layer = new _storagelayer.StorageLayer(this.storage);
        _class_private_field_get(this, _storages).push(layer);
        return layer;
    }
    /**
   * Pop a layer from the storage stack.
   */ popStorageLayer() {
        _class_private_field_get(this, _storages).pop();
    }
    /**
   * Truncate the storage layer stack back to a target depth.
   *
   * Used by snapshot-restore mechanisms to undo accumulated dev.setStorage
   * calls when reverting to a previously captured state, without losing the
   * underlying block.
   */ resetStorageLayers(targetCount) {
        while(_class_private_field_get(this, _storages).length > targetCount)_class_private_field_get(this, _storages).pop();
    }
    /** The current depth of the storage layer stack. */ get storageLayerCount() {
        return _class_private_field_get(this, _storages).length;
    }
    /**
   * Get storage diff.
   */ async storageDiff() {
        const storage = {};
        for (const layer of _class_private_field_get(this, _storages)){
            await layer.mergeInto(storage);
        }
        return storage;
    }
    /**
   * Get the wasm string.
   */ get wasm() {
        if (!_class_private_field_get(this, _wasm)) {
            _class_private_field_set(this, _wasm, (async ()=>{
                const wasmKey = (0, _util.stringToHex)(':code');
                const wasm = await this.get(wasmKey);
                if (!wasm) {
                    throw new Error('No wasm found');
                }
                return wasm;
            })());
        }
        return _class_private_field_get(this, _wasm);
    }
    /**
   * Set the runtime wasm.
   */ setWasm(wasm) {
        const wasmKey = (0, _util.stringToHex)(':code');
        this.pushStorageLayer().set(wasmKey, wasm);
        _class_private_field_set(this, _wasm, Promise.resolve(wasm));
        _class_private_field_set(this, _runtimeVersion, undefined);
        _class_private_field_set(this, _registry, undefined);
        _class_private_field_set(this, _meta, undefined);
        _class_private_field_set(this, _metadata1, undefined);
    }
    /**
   * Get the type registry.
   * @see https://polkadot.js.org/docs/api/start/types.create#why-create-types
   */ get registry() {
        if (!_class_private_field_get(this, _registry)) {
            _class_private_field_set(this, _registry, Promise.all([
                this.metadata,
                this.runtimeVersion
            ]).then(([data, version])=>_class_private_field_get(this, _chain).buildRegistry(data, version)));
        }
        return _class_private_field_get(this, _registry);
    }
    get runtimeVersion() {
        if (!_class_private_field_get(this, _runtimeVersion)) {
            _class_private_field_set(this, _runtimeVersion, this.wasm.then(_index1.getRuntimeVersion));
        }
        return _class_private_field_get(this, _runtimeVersion);
    }
    get metadata() {
        if (!_class_private_field_get(this, _metadata1)) {
            _class_private_field_set(this, _metadata1, this.call('Metadata_metadata', []).then((resp)=>(0, _index.compactHex)((0, _util.hexToU8a)(resp.result))));
        }
        return _class_private_field_get(this, _metadata1);
    }
    get meta() {
        if (!_class_private_field_get(this, _meta)) {
            _class_private_field_set(this, _meta, Promise.all([
                this.registry,
                this.metadata
            ]).then(([registry, metadataStr])=>{
                const metadata = new _types.Metadata(registry, metadataStr);
                return (0, _metadata.expandMetadata)(registry, metadata);
            }));
        }
        return _class_private_field_get(this, _meta);
    }
    /**
   * Call a runtime method.
   */ async call(method, args, mockSigantureHostOverride = false) {
        const wasm = await this.wasm;
        const response = await (0, _index1.runTask)({
            wasm,
            calls: [
                [
                    method,
                    args
                ]
            ],
            mockSignatureHost: _class_private_field_get(this, _chain).mockSignatureHost,
            allowUnresolvedImports: _class_private_field_get(this, _chain).allowUnresolvedImports,
            runtimeLogLevel: _class_private_field_get(this, _chain).runtimeLogLevel
        }, (0, _index1.taskHandler)(this), mockSigantureHostOverride);
        if ('Call' in response) {
            if (this.chain.offchainWorker) {
                // apply offchain storage
                for (const [key, value] of response.Call.offchainStorageDiff){
                    this.chain.offchainWorker.set(key, value);
                }
            }
            return response.Call;
        }
        if (response.Error) throw Error(response.Error);
        throw Error('Unexpected response');
    }
    constructor(chain, number, hash, parentBlock, block){
        _define_property(this, "number", void 0);
        _define_property(this, "hash", void 0);
        _class_private_field_init(this, _chain, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _header, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _parentBlock, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _extrinsics, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _wasm, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _runtimeVersion, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _metadata1, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _registry, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _meta, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _baseStorage, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _storages, {
            writable: true,
            value: void 0
        });
        this.number = number;
        this.hash = hash;
        _class_private_field_set(this, _chain, chain);
        _class_private_field_set(this, _parentBlock, parentBlock ? new WeakRef(parentBlock) : undefined);
        _class_private_field_set(this, _header, block?.header);
        _class_private_field_set(this, _extrinsics, block?.extrinsics);
        _class_private_field_set(this, _baseStorage, block?.storage ?? new _storagelayer.RemoteStorageLayer(chain.api, hash, chain.db));
        _class_private_field_set(this, _storages, []);
        _class_private_field_set(this, _runtimeVersion, parentBlock?.runtimeVersion);
        _class_private_field_set(this, _metadata1, parentBlock?.metadata);
        _class_private_field_set(this, _registry, parentBlock?.registry);
        _class_private_field_set(this, _meta, parentBlock?.meta);
        const storageDiff = block?.storageDiff;
        if (storageDiff) {
            // if code doesn't change then keep parent block's meta
            // otherwise reset meta
            if (storageDiff[(0, _util.stringToHex)(':code')]) {
                _class_private_field_set(this, _runtimeVersion, undefined);
                _class_private_field_set(this, _metadata1, undefined);
                _class_private_field_set(this, _registry, undefined);
                _class_private_field_set(this, _meta, undefined);
            }
            this.pushStorageLayer().setAll(storageDiff);
        }
    }
}
