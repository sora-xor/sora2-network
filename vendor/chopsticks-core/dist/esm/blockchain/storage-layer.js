import _ from 'lodash';
import { defaultLogger } from '../logger.js';
import { CHILD_PREFIX_LENGTH, isPrefixedChildKey, PREFIX_LENGTH } from '../utils/index.js';
import KeyCache from '../utils/key-cache.js';
const logger = defaultLogger.child({
    name: 'layer'
});
const BATCH_SIZE = 1000;
export var StorageValueKind = /*#__PURE__*/ function(StorageValueKind) {
    StorageValueKind["Deleted"] = "Deleted";
    StorageValueKind["DeletedPrefix"] = "DeletedPrefix";
    return StorageValueKind;
}({});
export class RemoteStorageLayer {
    #api;
    #at;
    #db;
    #keyCache = new KeyCache(PREFIX_LENGTH);
    #defaultChildKeyCache = new KeyCache(CHILD_PREFIX_LENGTH);
    #inflight = new Map();
    constructor(api, at, db){
        this.#api = api;
        this.#at = at;
        this.#db = db;
    }
    deleted(_key) {
        return false;
    }
    async get(key, _cache) {
        if (this.#db) {
            const res = await this.#db.queryStorage(this.#at, key);
            if (res) {
                return res.value ?? undefined;
            }
        }
        const inflight = this.#inflight.get(key);
        if (inflight) return inflight;
        logger.trace({
            at: this.#at,
            key
        }, 'RemoteStorageLayer get');
        const fetch = this.#api.getStorage(key, this.#at).then((data)=>{
            this.#db?.saveStorage(this.#at, key, data);
            return data ?? undefined;
        }).finally(()=>{
            this.#inflight.delete(key);
        });
        this.#inflight.set(key, fetch);
        return fetch;
    }
    async getMany(keys, _cache) {
        const result = [];
        let pending = keys.map((key, idx)=>({
                key,
                idx
            }));
        if (this.#db) {
            const results = await Promise.all(pending.map(({ key })=>this.#db.queryStorage(this.#at, key)));
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
                at: this.#at,
                keys
            }, 'RemoteStorageLayer getMany');
            const data = await this.#api.getStorageBatch('0x', pending.map(({ key })=>key), this.#at);
            data.forEach(([, res], idx)=>{
                result[pending[idx].idx] = res ?? undefined;
            });
            if (this.#db?.saveStorageBatch) {
                this.#db?.saveStorageBatch(data.map(([key, value])=>({
                        key,
                        value,
                        blockHash: this.#at
                    })));
            } else if (this.#db) {
                data.forEach(([key, value])=>{
                    this.#db?.saveStorage(this.#at, key, value);
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
            at: this.#at,
            prefix,
            pageSize,
            startKey
        }, 'RemoteStorageLayer getKeysPaged');
        const isChild = isPrefixedChildKey(prefix);
        const minPrefixLen = isChild ? CHILD_PREFIX_LENGTH : PREFIX_LENGTH;
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
            return this.#api.getKeysPaged(prefix, pageSize, startKey, this.#at);
        }
        const startKeyEqualsPrefix = startKey === prefix;
        let cachedKeys;
        if (this.#db?.queryPagedKeys && startKeyEqualsPrefix) {
            const cached = await this.#db.queryPagedKeys(this.#at, prefix);
            if (cached) {
                cachedKeys = cached;
                isChild ? this.#defaultChildKeyCache.feed([
                    startKey,
                    ...cached
                ]) : this.#keyCache.feed([
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
            const nextKey = isChild ? await this.#defaultChildKeyCache.next(startKey) : await this.#keyCache.next(startKey);
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
            const batch = await this.#api.getKeysPaged(prefix, BATCH_SIZE, startKey, this.#at);
            batchComplete = batch.length < BATCH_SIZE;
            // feed the key cache
            if (batch.length > 0) {
                if (isChild) {
                    this.#defaultChildKeyCache.feed([
                        startKey,
                        ...batch
                    ]);
                } else {
                    this.#keyCache.feed([
                        startKey,
                        ...batch
                    ]);
                }
            }
            if (batch.length === 0) {
                break;
            }
            if (this.#db) {
                const newBatch = await Promise.all(batch.map((key)=>this.#db.queryStorage(this.#at, key).then((r)=>r ? null : key))).then((rs)=>rs.filter((k)=>k !== null));
                if (newBatch.length > 0) {
                    // batch fetch storage values and save to db, they may be used later
                    this.#api.getStorageBatch(prefix, newBatch, this.#at).then((storage)=>{
                        for (const [key, value] of storage){
                            this.#db?.saveStorage(this.#at, key, value);
                        }
                    });
                }
                if (startKeyEqualsPrefix) {
                    allFetchedKeys.push(...batch);
                    fetchedNewKeys = true;
                }
            }
        }
        if (this.#db?.savePagedKeys && startKeyEqualsPrefix && fetchedNewKeys) {
            await this.#db.savePagedKeys(this.#at, prefix, allFetchedKeys);
        }
        return keysPaged;
    }
}
export class StorageLayer {
    #store = new Map();
    #keys = [];
    #deletedPrefix = [];
    #parent;
    constructor(parent){
        this.#parent = parent;
    }
    #addKey(key) {
        const idx = _.sortedIndex(this.#keys, key);
        const key2 = this.#keys[idx];
        if (key === key2) {
            return;
        }
        this.#keys.splice(idx, 0, key);
    }
    #removeKey(key) {
        const idx = _.sortedIndex(this.#keys, key);
        const key2 = this.#keys[idx];
        if (key === key2) {
            this.#keys.splice(idx, 1);
        }
    }
    #cacheParentValue(key, value) {
        if (!this.#store.has(key) && !this.#deletedPrefix.some((dp)=>key.startsWith(dp))) {
            this.#store.set(key, value);
        }
    }
    deleted(key) {
        if (this.#store.has(key)) {
            return this.#store.get(key) === "Deleted";
        }
        if (this.#deletedPrefix.some((dp)=>key.startsWith(dp))) {
            return true;
        }
        if (this.#parent) {
            return this.#parent.deleted(key);
        }
        return false;
    }
    async get(key, cache) {
        if (this.#store.has(key)) {
            return this.#store.get(key);
        }
        if (this.#deletedPrefix.some((dp)=>key.startsWith(dp))) {
            return "Deleted";
        }
        if (this.#parent) {
            const val = await this.#parent.get(key, false);
            if (cache) {
                this.#cacheParentValue(key, val);
            }
            return val;
        }
        return undefined;
    }
    async getMany(keys, cache) {
        const result = [];
        const pending = [];
        keys.forEach((key, idx)=>{
            if (this.#store.has(key)) {
                result[idx] = this.#store.get(key);
            } else if (this.#deletedPrefix.some((dp)=>key.startsWith(dp))) {
                result[idx] = "Deleted";
            } else {
                pending.push({
                    key,
                    idx
                });
            }
        });
        if (pending.length && this.#parent) {
            const vals = await this.#parent.getMany(pending.map((p)=>p.key), false);
            vals.forEach((val, idx)=>{
                if (cache) {
                    this.#cacheParentValue(pending[idx].key, val);
                }
                result[pending[idx].idx] = val;
            });
        }
        return result;
    }
    set(key, value) {
        switch(value){
            case "Deleted":
                this.#store.set(key, "Deleted");
                this.#removeKey(key);
                break;
            case "DeletedPrefix":
                this.#deletedPrefix.push(key);
                for (const k of this.#store.keys()){
                    if (k.startsWith(key)) {
                        this.#store.set(k, "Deleted");
                        this.#removeKey(k);
                    }
                }
                break;
            case undefined:
                this.#store.delete(key);
                this.#removeKey(key);
                break;
            default:
                this.#store.set(key, value);
                this.#addKey(key);
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
        const maybeBest = this.#keys.find((key)=>key.startsWith(prefix) && key > startKey);
        if (!knownBest) {
            knownBest = maybeBest;
        } else if (maybeBest && maybeBest < knownBest) {
            knownBest = maybeBest;
        }
        if (this.#parent && !this.#deletedPrefix.some((dp)=>dp === prefix)) {
            const parentBest = await this.#parent.findNextKey(prefix, startKey, knownBest);
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
        for (const [key, value] of this.#store){
            if (value === "Deleted") {
                into[key] = null;
            } else {
                into[key] = value;
            }
        }
    }
}
