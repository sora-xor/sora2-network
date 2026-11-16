"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "GenesisProvider", {
    enumerable: true,
    get: function() {
        return GenesisProvider;
    }
});
const _eventemitter3 = require("eventemitter3");
const _index = require("./index.js");
const _index1 = require("./schema/index.js");
const _index2 = require("./wasm-executor/index.js");
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
var _isConnected = /*#__PURE__*/ new WeakMap(), _eventemitter = /*#__PURE__*/ new WeakMap(), _isReadyPromise = /*#__PURE__*/ new WeakMap(), _genesis = /*#__PURE__*/ new WeakMap(), _stateRoot = /*#__PURE__*/ new WeakMap();
class GenesisProvider {
    get isClonable() {
        return true;
    }
    get hasSubscriptions() {
        return false;
    }
    get isConnected() {
        return _class_private_field_get(this, _isConnected);
    }
    get isReady() {
        return _class_private_field_get(this, _isReadyPromise);
    }
    get blockHash() {
        return '0x4545454545454545454545454545454545454545454545454545454545454545';
    }
    get _jsCallback() {
        const storage = _class_private_field_get(this, _genesis).genesis.raw.top;
        return {
            ..._index2.emptyTaskHandler,
            getStorage: async (key)=>{
                if ((0, _index.isPrefixedChildKey)(key)) {
                    _index.defaultLogger.warn({
                        key
                    }, 'genesis child storage not supported');
                    return undefined;
                }
                return storage[key];
            },
            getNextKey: async (prefix, key)=>{
                if ((0, _index.isPrefixedChildKey)(key)) {
                    _index.defaultLogger.warn({
                        prefix,
                        key
                    }, 'genesis child storage not supported');
                    return undefined;
                }
                return Object.keys(storage).find((k)=>{
                    if (!k.startsWith(prefix)) return false;
                    if (key.length > prefix.length) {
                        return k > key;
                    }
                    return true;
                });
            }
        };
    }
    /**
   * @ignore
   * Create a genesis provider
   *
   * @param genesis - genesis file
   * @requires genesis provider
   */ constructor(genesis){
        _class_private_field_init(this, _isConnected, {
            writable: true,
            value: false
        });
        _class_private_field_init(this, _eventemitter, {
            writable: true,
            value: new _eventemitter3.EventEmitter()
        });
        _class_private_field_init(this, _isReadyPromise, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _genesis, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _stateRoot, {
            writable: true,
            value: void 0
        });
        _define_property(this, "genesisHeaderLogs", []);
        _define_property(this, "clone", ()=>{
            return new GenesisProvider(_class_private_field_get(this, _genesis));
        });
        _define_property(this, "connect", async ()=>{
            _class_private_field_set(this, _isConnected, true);
            _class_private_field_get(this, _eventemitter).emit('connected');
        });
        _define_property(this, "disconnect", async ()=>{
            _class_private_field_set(this, _isConnected, false);
            _class_private_field_get(this, _eventemitter).emit('disconnected');
        });
        _define_property(this, "on", (type, sub)=>{
            _class_private_field_get(this, _eventemitter).on(type, sub);
            return ()=>{
                _class_private_field_get(this, _eventemitter).removeListener(type, sub);
            };
        });
        _define_property(this, "getHeader", async ()=>{
            return {
                number: '0x0',
                stateRoot: await _class_private_field_get(this, _stateRoot),
                parentHash: '0x4545454545454545454545454545454545454545454545454545454545454545',
                extrinsicsRoot: '0x03170a2e7597b7b7e3d84c05391d139a62b157e78786d8c082f29dcf4c111314',
                digest: {
                    logs: this.genesisHeaderLogs
                }
            };
        });
        _define_property(this, "getBlock", async ()=>{
            return {
                block: {
                    header: await this.getHeader(),
                    extrinsics: []
                }
            };
        });
        _define_property(this, "send", async (method, params, _isCacheable)=>{
            await this.isReady;
            switch(method){
                case 'system_properties':
                    return _class_private_field_get(this, _genesis).properties;
                case 'system_chain':
                    return _class_private_field_get(this, _genesis).id;
                case 'system_name':
                    return _class_private_field_get(this, _genesis).name;
                case 'chain_getHeader':
                    return this.getHeader();
                case 'chain_getBlock':
                    return this.getBlock();
                case 'chain_getBlockHash':
                case 'chain_getFinalizedHead':
                    return this.blockHash;
                case 'state_getKeysPaged':
                case 'state_getKeysPagedAt':
                    {
                        if (params.length < 2) throw Error('invalid params');
                        const [prefix, size, start] = params;
                        let startKey = start || prefix;
                        const keys = [];
                        while(keys.length < size){
                            const nextKey = await this._jsCallback.getNextKey(prefix, startKey);
                            if (!nextKey) break;
                            keys.push(nextKey);
                            startKey = nextKey;
                        }
                        return keys;
                    }
                case 'state_getStorage':
                case 'state_getStorageAt':
                    {
                        if (params.length < 1) throw Error('invalid params');
                        return _class_private_field_get(this, _genesis).genesis.raw.top[params[0]];
                    }
                case 'state_queryStorageAt':
                    {
                        if (params.length < 2) throw Error('invalid params');
                        const [keys, hash] = params;
                        const values = [];
                        for (const key of keys){
                            const storage = _class_private_field_get(this, _genesis).genesis.raw.top[key];
                            if (storage) {
                                values.push(storage);
                            }
                        }
                        return {
                            block: hash,
                            changes: values
                        };
                    }
                default:
                    throw Error(`${method} not implemented`);
            }
        });
        _define_property(this, "subscribe", async (_type, _method, _params, _cb)=>{
            throw Error('unimplemented');
        });
        _define_property(this, "unsubscribe", async (_type, _method, _id)=>{
            throw Error('unimplemented');
        });
        _class_private_field_set(this, _genesis, _index1.genesisSchema.parse(genesis));
        _class_private_field_set(this, _stateRoot, (0, _index2.calculateStateRoot)(Object.entries(_class_private_field_get(this, _genesis).genesis.raw.top).reduce((accu, item)=>{
            accu.push(item);
            return accu;
        }, []), 1));
        _class_private_field_set(this, _isReadyPromise, new Promise((resolve, reject)=>{
            _class_private_field_get(this, _eventemitter).once('connected', ()=>{
                resolve();
            });
            _class_private_field_get(this, _eventemitter).once('error', reject);
            this.connect();
        }));
    }
}
