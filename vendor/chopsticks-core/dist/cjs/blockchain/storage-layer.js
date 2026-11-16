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
    get RemoteStorageLayer () {
        return RemoteStorageLayer;
    },
    get StorageLayer () {
        return StorageLayer;
    },
    get StorageValueKind () {
        return StorageValueKind;
    }
});
const _lodash = /*#__PURE__*/ _interop_require_default(require("lodash"));
const _logger = require("../logger.js");
const _index = require("../utils/index.js");
const _keycache = /*#__PURE__*/ _interop_require_default(require("../utils/key-cache.js"));
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
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const logger = _logger.defaultLogger.child({
    name: 'layer'
});
const BATCH_SIZE = 1000;
var StorageValueKind = /*#__PURE__*/ function(StorageValueKind) {
    StorageValueKind["Deleted"] = "Deleted";
    StorageValueKind["DeletedPrefix"] = "DeletedPrefix";
    return StorageValueKind;
}({});
var _api = /*#__PURE__*/ new WeakMap(), _at = /*#__PURE__*/ new WeakMap(), _db = /*#__PURE__*/ new WeakMap(), _keyCache = /*#__PURE__*/ new WeakMap(), _defaultChildKeyCache = /*#__PURE__*/ new WeakMap(), _inflight = /*#__PURE__*/ new WeakMap();
class RemoteStorageLayer {
    deleted(_key) {
        return false;
    }
    async get(key, _cache) {
        if (_class_private_field_get(this, _db)) {
            const res = await _class_private_field_get(this, _db).queryStorage(_class_private_field_get(this, _at), key);
            if (res) {
                return res.value ?? undefined;
            }
        }
        const inflight = _class_private_field_get(this, _inflight).get(key);
        if (inflight) return inflight;
        logger.trace({
            at: _class_private_field_get(this, _at),
            key
        }, 'RemoteStorageLayer get');
        const fetch = _class_private_field_get(this, _api).getStorage(key, _class_private_field_get(this, _at)).then((data)=>{
            _class_private_field_get(this, _db)?.saveStorage(_class_private_field_get(this, _at), key, data);
            return data ?? undefined;
        }).finally(()=>{
            _class_private_field_get(this, _inflight).delete(key);
        });
        _class_private_field_get(this, _inflight).set(key, fetch);
        return fetch;
    }
    async getMany(keys, _cache) {
        const result = [];
        let pending = keys.map((key, idx)=>({
                key,
                idx
            }));
        if (_class_private_field_get(this, _db)) {
            const results = await Promise.all(pending.map(({ key })=>_class_private_field_get(this, _db).queryStorage(_class_private_field_get(this, _at), key)));
            const oldPending = pending;
            pending = [];
            results.forEach((res, idx)=>{
                if (res) {
                    result[idx] = res.value ?? undefined;
                } else {
                    pending.push({
                        key: oldPending[idx].key,
                        idx
                    });
                }
            });
        }
        if (pending.length) {
            logger.trace({
                at: _class_private_field_get(this, _at),
                keys
            }, 'RemoteStorageLayer getMany');
            const data = await _class_private_field_get(this, _api).getStorageBatch('0x', pending.map(({ key })=>key), _class_private_field_get(this, _at));
            data.forEach(([, res], idx)=>{
                result[pending[idx].idx] = res ?? undefined;
            });
            if (_class_private_field_get(this, _db)?.saveStorageBatch) {
                _class_private_field_get(this, _db)?.saveStorageBatch(data.map(([key, value])=>({
                        key,
                        value,
                        blockHash: _class_private_field_get(this, _at)
                    })));
            } else if (_class_private_field_get(this, _db)) {
                data.forEach(([key, value])=>{
                    _class_private_field_get(this, _db)?.saveStorage(_class_private_field_get(this, _at), key, value);
                });
            }
        }
        return result;
    }
    async findNextKey(prefix, startKey, _knownBest) {
        const keys = await this.getKeysPaged(prefix, 1, startKey);
        return keys[0];
    }
    async getKeysPaged(prefix, pageSize, startKey) {
        if (pageSize > BATCH_SIZE) throw new Error(`pageSize must be less or equal to ${BATCH_SIZE}`);
        logger.trace({
            at: _class_private_field_get(this, _at),
            prefix,
            pageSize,
            startKey
        }, 'RemoteStorageLayer getKeysPaged');
        const isChild = (0, _index.isPrefixedChildKey)(prefix);
        const minPrefixLen = isChild ? _index.CHILD_PREFIX_LENGTH : _index.PREFIX_LENGTH;
        // KeyCache groups by the first `minPrefixLen` chars; it cannot correctly answer queries whose PREFIX is
        // off that grouping width, so those are proxied directly to upstream (a longer prefix would otherwise match
        // on the truncated head and return keys that do not start with the requested prefix — smoldot's
        // `clear_prefix` asserts `key.starts_with(prefix)`).
        //
        // `startKey` is a different matter: every CONTINUATION of a scan carries a full-length storage key by
        // construction, and serving those from the cache is precisely what the cache is for. Bailing out on a long
        // `startKey` makes each continuation page an upstream round-trip.
        //
        // But a long `startKey` is only cache-safe when it belongs to the SAME group as `prefix`. `KeyCache.next()`
        // derives its range from the first `minPrefixLen` chars of the `startKey` alone and never checks the result
        // against `prefix`, so a `startKey` from a different `minPrefixLen`-char group would be answered from that
        // other group's range — returning a key that does not start with `prefix`. `!startKey.startsWith(prefix)`
        // proxies those cross-group requests to upstream; genuine continuations (`startKey.startsWith(prefix)` by
        // construction) still hit the cache. (Reported by @xlc on #1055.)
        if (prefix.length !== minPrefixLen || startKey.length < minPrefixLen || !startKey.startsWith(prefix)) {
            return _class_private_field_get(this, _api).getKeysPaged(prefix, pageSize, startKey, _class_private_field_get(this, _at));
        }
        const startKeyEqualsPrefix = startKey === prefix;
        let cachedKeys;
        if (_class_private_field_get(this, _db)?.queryPagedKeys && startKeyEqualsPrefix) {
            const cached = await _class_private_field_get(this, _db).queryPagedKeys(_class_private_field_get(this, _at), prefix);
            if (cached) {
                cachedKeys = cached;
                isChild ? _class_private_field_get(this, _defaultChildKeyCache).feed([
                    startKey,
                    ...cached
                ]) : _class_private_field_get(this, _keyCache).feed([
                    startKey,
                    ...cached
                ]);
            }
        }
        let batchComplete = false;
        let fetchedNewKeys = false;
        const keysPaged = [];
        // Seed with cached keys so a cache-hit-then-extend doesn't truncate the persisted set.
        const allFetchedKeys = cachedKeys ? [
            ...cachedKeys
        ] : [];
        while(keysPaged.length < pageSize){
            const nextKey = isChild ? await _class_private_field_get(this, _defaultChildKeyCache).next(startKey) : await _class_private_field_get(this, _keyCache).next(startKey);
            if (nextKey) {
                keysPaged.push(nextKey);
                startKey = nextKey;
                continue;
            }
            // batch fetch was completed
            if (batchComplete) {
                break;
            }
            // fetch a batch of keys
            const batch = await _class_private_field_get(this, _api).getKeysPaged(prefix, BATCH_SIZE, startKey, _class_private_field_get(this, _at));
            batchComplete = batch.length < BATCH_SIZE;
            // feed the key cache
            if (batch.length > 0) {
                if (isChild) {
                    _class_private_field_get(this, _defaultChildKeyCache).feed([
                        startKey,
                        ...batch
                    ]);
                } else {
                    _class_private_field_get(this, _keyCache).feed([
                        startKey,
                        ...batch
                    ]);
                }
            }
            if (batch.length === 0) {
                break;
            }
            if (_class_private_field_get(this, _db)) {
                const newBatch = await Promise.all(batch.map((key)=>_class_private_field_get(this, _db).queryStorage(_class_private_field_get(this, _at), key).then((r)=>r ? null : key))).then((rs)=>rs.filter((k)=>k !== null));
                if (newBatch.length > 0) {
                    // batch fetch storage values and save to db, they may be used later
                    _class_private_field_get(this, _api).getStorageBatch(prefix, newBatch, _class_private_field_get(this, _at)).then((storage)=>{
                        for (const [key, value] of storage){
                            _class_private_field_get(this, _db)?.saveStorage(_class_private_field_get(this, _at), key, value);
                        }
                    });
                }
                if (startKeyEqualsPrefix) {
                    allFetchedKeys.push(...batch);
                    fetchedNewKeys = true;
                }
            }
        }
        if (_class_private_field_get(this, _db)?.savePagedKeys && startKeyEqualsPrefix && fetchedNewKeys) {
            await _class_private_field_get(this, _db).savePagedKeys(_class_private_field_get(this, _at), prefix, allFetchedKeys);
        }
        return keysPaged;
    }
    constructor(api, at, db){
        _class_private_field_init(this, _api, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _at, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _db, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _keyCache, {
            writable: true,
            value: new _keycache.default(_index.PREFIX_LENGTH)
        });
        _class_private_field_init(this, _defaultChildKeyCache, {
            writable: true,
            value: new _keycache.default(_index.CHILD_PREFIX_LENGTH)
        });
        _class_private_field_init(this, _inflight, {
            writable: true,
            value: new Map()
        });
        _class_private_field_set(this, _api, api);
        _class_private_field_set(this, _at, at);
        _class_private_field_set(this, _db, db);
    }
}
var _store = /*#__PURE__*/ new WeakMap(), _keys = /*#__PURE__*/ new WeakMap(), _deletedPrefix = /*#__PURE__*/ new WeakMap(), _parent = /*#__PURE__*/ new WeakMap(), _addKey = /*#__PURE__*/ new WeakSet(), _removeKey = /*#__PURE__*/ new WeakSet(), _cacheParentValue = /*#__PURE__*/ new WeakSet();
class StorageLayer {
    deleted(key) {
        if (_class_private_field_get(this, _store).has(key)) {
            return _class_private_field_get(this, _store).get(key) === "Deleted";
        }
        if (_class_private_field_get(this, _deletedPrefix).some((dp)=>key.startsWith(dp))) {
            return true;
        }
        if (_class_private_field_get(this, _parent)) {
            return _class_private_field_get(this, _parent).deleted(key);
        }
        return false;
    }
    async get(key, cache) {
        if (_class_private_field_get(this, _store).has(key)) {
            return _class_private_field_get(this, _store).get(key);
        }
        if (_class_private_field_get(this, _deletedPrefix).some((dp)=>key.startsWith(dp))) {
            return "Deleted";
        }
        if (_class_private_field_get(this, _parent)) {
            const val = await _class_private_field_get(this, _parent).get(key, false);
            if (cache) {
                _class_private_method_get(this, _cacheParentValue, cacheParentValue).call(this, key, val);
            }
            return val;
        }
        return undefined;
    }
    async getMany(keys, cache) {
        const result = [];
        const pending = [];
        keys.forEach((key, idx)=>{
            if (_class_private_field_get(this, _store).has(key)) {
                result[idx] = _class_private_field_get(this, _store).get(key);
            } else if (_class_private_field_get(this, _deletedPrefix).some((dp)=>key.startsWith(dp))) {
                result[idx] = "Deleted";
            } else {
                pending.push({
                    key,
                    idx
                });
            }
        });
        if (pending.length && _class_private_field_get(this, _parent)) {
            const vals = await _class_private_field_get(this, _parent).getMany(pending.map((p)=>p.key), false);
            vals.forEach((val, idx)=>{
                if (cache) {
                    _class_private_method_get(this, _cacheParentValue, cacheParentValue).call(this, pending[idx].key, val);
                }
                result[pending[idx].idx] = val;
            });
        }
        return result;
    }
    set(key, value) {
        switch(value){
            case "Deleted":
                _class_private_field_get(this, _store).set(key, "Deleted");
                _class_private_method_get(this, _removeKey, removeKey).call(this, key);
                break;
            case "DeletedPrefix":
                _class_private_field_get(this, _deletedPrefix).push(key);
                for (const k of _class_private_field_get(this, _store).keys()){
                    if (k.startsWith(key)) {
                        _class_private_field_get(this, _store).set(k, "Deleted");
                        _class_private_method_get(this, _removeKey, removeKey).call(this, k);
                    }
                }
                break;
            case undefined:
                _class_private_field_get(this, _store).delete(key);
                _class_private_method_get(this, _removeKey, removeKey).call(this, key);
                break;
            default:
                _class_private_field_get(this, _store).set(key, value);
                _class_private_method_get(this, _addKey, addKey).call(this, key);
                break;
        }
    }
    setAll(values) {
        if (!Array.isArray(values)) {
            values = Object.entries(values);
        }
        for (const [key, value] of values){
            this.set(key, value || "Deleted");
        }
    }
    async findNextKey(prefix, startKey, knownBest) {
        const maybeBest = _class_private_field_get(this, _keys).find((key)=>key.startsWith(prefix) && key > startKey);
        if (!knownBest) {
            knownBest = maybeBest;
        } else if (maybeBest && maybeBest < knownBest) {
            knownBest = maybeBest;
        }
        if (_class_private_field_get(this, _parent) && !_class_private_field_get(this, _deletedPrefix).some((dp)=>dp === prefix)) {
            const parentBest = await _class_private_field_get(this, _parent).findNextKey(prefix, startKey, knownBest);
            if (parentBest) {
                if (!maybeBest) {
                    return parentBest;
                }
                if (parentBest < maybeBest) {
                    return parentBest;
                }
            }
        }
        return knownBest;
    }
    async getKeysPaged(prefix, pageSize, startKey) {
        if (pageSize > BATCH_SIZE) throw new Error(`pageSize must be less or equal to ${BATCH_SIZE}`);
        if (!startKey || startKey === '0x') {
            startKey = prefix;
        }
        const keys = [];
        while(keys.length < pageSize){
            const next = await this.findNextKey(prefix, startKey, undefined);
            if (!next) break;
            startKey = next;
            if (this.deleted(next)) continue;
            keys.push(next);
        }
        return keys;
    }
    /**
   * Merge the storage layer into the given object, can be used to get sotrage diff.
   */ async mergeInto(into) {
        for (const [key, value] of _class_private_field_get(this, _store)){
            if (value === "Deleted") {
                into[key] = null;
            } else {
                into[key] = value;
            }
        }
    }
    constructor(parent){
        _class_private_method_init(this, _addKey);
        _class_private_method_init(this, _removeKey);
        _class_private_method_init(this, _cacheParentValue);
        _class_private_field_init(this, _store, {
            writable: true,
            value: new Map()
        });
        _class_private_field_init(this, _keys, {
            writable: true,
            value: []
        });
        _class_private_field_init(this, _deletedPrefix, {
            writable: true,
            value: []
        });
        _class_private_field_init(this, _parent, {
            writable: true,
            value: void 0
        });
        _class_private_field_set(this, _parent, parent);
    }
}
function addKey(key) {
    const idx = _lodash.default.sortedIndex(_class_private_field_get(this, _keys), key);
    const key2 = _class_private_field_get(this, _keys)[idx];
    if (key === key2) {
        return;
    }
    _class_private_field_get(this, _keys).splice(idx, 0, key);
}
function removeKey(key) {
    const idx = _lodash.default.sortedIndex(_class_private_field_get(this, _keys), key);
    const key2 = _class_private_field_get(this, _keys)[idx];
    if (key === key2) {
        _class_private_field_get(this, _keys).splice(idx, 1);
    }
}
function cacheParentValue(key, value) {
    if (!_class_private_field_get(this, _store).has(key) && !_class_private_field_get(this, _deletedPrefix).some((dp)=>key.startsWith(dp))) {
        _class_private_field_get(this, _store).set(key, value);
    }
}
