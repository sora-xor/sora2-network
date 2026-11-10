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
    get decodeBlockStorageDiff () {
        return decodeBlockStorageDiff;
    },
    get decodeKey () {
        return decodeKey;
    },
    get decodeKeyValue () {
        return decodeKeyValue;
    },
    get toStorageObject () {
        return toStorageObject;
    }
});
require("@polkadot/types-codec");
const _util = require("@polkadot/util");
const _lodash = /*#__PURE__*/ _interop_require_default(require("lodash"));
const _lrucache = require("lru-cache");
const _logger = require("../logger.js");
const _wellknownkeys = require("./well-known-keys.js");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const logger = _logger.defaultLogger.child({
    name: 'decoder'
});
const _CACHE = new _lrucache.LRUCache({
    max: 20 /* max 20 registries */ 
});
const getCache = (registry)=>{
    const cache = _CACHE.get(registry);
    if (cache) {
        return cache;
    }
    const newCache = new _lrucache.LRUCache({
        max: 100 /* max 100 storage entries */ 
    });
    _CACHE.set(registry, newCache);
    return newCache;
};
const getStorageEntry = (meta, key)=>{
    const cache = getCache(meta.registry);
    for (const prefix of cache.keys()){
        if (key.startsWith(prefix)) // update the recency of the cache entry
        return cache.get(prefix);
    }
    for (const module of Object.values(meta.query)){
        for (const storage of Object.values(module)){
            const keyPrefix = (0, _util.u8aToHex)(storage.keyPrefix());
            if (key.startsWith(keyPrefix)) {
                cache.set(keyPrefix, storage);
                return storage;
            }
        }
    }
    return undefined;
};
const decodeKey = (meta, key)=>{
    const storage = getStorageEntry(meta, key);
    const decodedKey = meta.registry.createType('StorageKey', key);
    if (storage) {
        decodedKey.setMeta(storage.meta);
        return {
            storage,
            decodedKey
        };
    }
    return {};
};
const decodeKeyValue = (meta, key, value, toHuman = true)=>{
    const res = (0, _wellknownkeys.decodeWellKnownKey)(meta.registry, key, value);
    if (res) {
        return {
            section: 'substrate',
            method: res.name,
            key: res.key,
            value: res.value
        };
    }
    const { storage, decodedKey } = decodeKey(meta, key);
    if (!storage || !decodedKey) {
        logger.warn({
            key,
            value
        }, 'Failed to decode storage key');
        return undefined;
    }
    const decodeValue = ()=>{
        if (!value) return null;
        try {
            return meta.registry.createType(decodedKey.outputType, (0, _util.hexToU8a)(value))[toHuman ? 'toHuman' : 'toJSON']();
        } catch (error) {
            logger.warn(error, 'Failed to decode storage value');
            logger.warn({
                key,
                value,
                section: storage.section,
                method: storage.method,
                args: decodedKey.args
            }, 'Failed to decode storage value');
            return undefined;
        }
    };
    return {
        section: storage.section,
        method: storage.method,
        key: decodedKey.args.map((x)=>x.toJSON()),
        value: decodeValue()
    };
};
const toStorageObject = (decoded)=>{
    if (!decoded) {
        return undefined;
    }
    const { section, method, key, value } = decoded;
    let obj = value;
    if (key) {
        for(let i = key.length - 1; i >= 0; i--){
            const k = key[i];
            const strKey = [
                'string',
                'number'
            ].includes(typeof k) ? k : JSON.stringify(k);
            const newObj = {
                [strKey]: obj
            };
            obj = newObj;
        }
    }
    return {
        [section]: {
            [method]: obj
        }
    };
};
const decodeBlockStorageDiff = async (block, diff)=>{
    const oldState = {};
    const newState = {};
    const meta = await block.meta;
    for (const [key, value] of diff){
        const oldValue = await block.get(key);
        const oldDecoded = toStorageObject(decodeKeyValue(meta, key, oldValue)) ?? {
            [key]: oldValue
        };
        _lodash.default.merge(oldState, oldDecoded);
        const newDecoded = toStorageObject(decodeKeyValue(meta, key, value)) ?? {
            [key]: value
        };
        _lodash.default.merge(newState, newDecoded);
    }
    return [
        oldState,
        newState
    ];
};
