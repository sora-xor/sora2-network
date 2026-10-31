import RpcError from '@polkadot/rpc-provider/coder/error';
import _ from 'lodash';
import { prefixedChildKey, splitChildKey, stripChildPrefix } from './utils/index.js';
const CACHEABLE_METHODS = new Set([
    'system_chain',
    'system_properties',
    'system_name',
    'chain_getBlockHash',
    'chain_getHeader'
]);
/**
 * API class. Calls provider to get on-chain data.
 * Either `endpoint` or `genesis` porvider must be provided.
 *
 * @example Instantiate an API
 *
 * ```ts
 * const provider = new WsProvider(options.endpoint)
 * const api = new Api(provider)
 * await api.isReady
 * ```
 */ export class Api {
    #provider;
    #ready;
    #chain;
    #chainProperties;
    #db;
    #scope;
    signedExtensions;
    #apiHooks = {};
    constructor(provider, signedExtensions){
        this.#provider = provider;
        this.signedExtensions = signedExtensions || {};
    }
    setDb(db, scope) {
        this.#db = db;
        this.#scope = scope;
    }
    async #cachedSend(method, params, isCacheable) {
        if (this.#db?.queryRpcCall && this.#scope && CACHEABLE_METHODS.has(method)) {
            const key = JSON.stringify(params);
            const cached = await this.#db.queryRpcCall(this.#scope, method, key);
            if (cached !== null) return JSON.parse(cached);
            this.#apiHooks?.fetching?.();
            const result = await this.#provider.send(method, params, isCacheable);
            await this.#db.saveRpcCall?.(this.#scope, method, key, JSON.stringify(result));
            return result;
        }
        this.#apiHooks?.fetching?.();
        return this.#provider.send(method, params, isCacheable);
    }
    async disconnect() {
        return this.#provider.disconnect();
    }
    get isReady() {
        if (!this.#ready) {
            if (this.#provider['isReady']) {
                this.#ready = this.#provider['isReady'];
            } else {
                this.#ready = new Promise((resolve)=>{
                    if (this.#provider.hasSubscriptions) {
                        this.#provider.on('connected', resolve);
                        this.#provider.connect();
                    } else {
                        resolve();
                    }
                });
            }
        }
        return this.#ready;
    }
    get chain() {
        if (!this.#chain) {
            this.#chain = this.getSystemChain();
        }
        return this.#chain;
    }
    get chainProperties() {
        if (!this.#chainProperties) {
            this.#chainProperties = this.getSystemProperties();
        }
        return this.#chainProperties;
    }
    // Set the hook function to be called when api fetch endpoint.
    onFetching(fetching) {
        this.#apiHooks.fetching = fetching;
    }
    async send(method, params, isCacheable) {
        this.#apiHooks?.fetching?.();
        return this.#provider.send(method, params, isCacheable);
    }
    async getSystemName() {
        return this.#cachedSend('system_name', []);
    }
    async getSystemProperties() {
        return this.#cachedSend('system_properties', []);
    }
    async getSystemChain() {
        return this.#cachedSend('system_chain', []);
    }
    async getBlockHash(blockNumber) {
        return this.#cachedSend('chain_getBlockHash', Number.isInteger(blockNumber) ? [
            blockNumber
        ] : [], !!blockNumber);
    }
    async getHeader(hash) {
        return this.#cachedSend('chain_getHeader', hash ? [
            hash
        ] : [], !!hash);
    }
    async getFinalizedHead() {
        return this.send('chain_getFinalizedHead', []);
    }
    async getBlock(hash) {
        return this.send('chain_getBlock', hash ? [
            hash
        ] : [], !!hash);
    }
    async getStorage(key, hash) {
        const [child, storageKey] = splitChildKey(key);
        if (child) {
            // child storage key, use childstate_getStorage
            const params = [
                child,
                storageKey
            ];
            if (hash) params.push(hash);
            return this.send('childstate_getStorage', params, !!hash);
        }
        // main storage key, use state_getStorage
        const params = [
            key
        ];
        if (hash) params.push(hash);
        return this.send('state_getStorage', params, !!hash);
    }
    async getKeysPaged(prefix, pageSize, startKey, hash) {
        const [child, storageKey] = splitChildKey(prefix);
        if (child) {
            // child storage key, use childstate_getKeysPaged
            // strip child prefix from startKey
            const params = [
                child,
                storageKey,
                pageSize,
                stripChildPrefix(startKey)
            ];
            if (hash) params.push(hash);
            return this.#provider.send('childstate_getKeysPaged', params, !!hash).then((keys)=>keys.map((key)=>prefixedChildKey(child, key)));
        }
        // main storage key, use state_getKeysPaged
        const params = [
            prefix,
            pageSize,
            startKey
        ];
        if (hash) params.push(hash);
        return this.send('state_getKeysPaged', params, !!hash);
    }
    async getStorageBatch(prefix, keys, hash) {
        // On response limit error, retry with a smaller batch size
        const retryOnError = async (ex)=>{
            if (ex instanceof RpcError && (ex.code === -32008 || ex.message === 'Response is too big')) {
                // Can't split beyond key size = 2
                if (keys.length < 2) throw ex;
                const mid = Math.floor(keys.length / 2);
                const batches = [
                    keys.slice(0, mid),
                    keys.slice(mid)
                ];
                const results = await Promise.all(batches.map((batch)=>this.getStorageBatch(prefix, batch, hash)));
                return results.flat();
            }
            throw ex;
        };
        const [child] = splitChildKey(prefix);
        if (child) {
            // child storage key, use childstate_getStorageEntries
            // strip child prefix from keys
            const params = [
                child,
                keys.map((key)=>stripChildPrefix(key))
            ];
            if (hash) params.push(hash);
            return this.#provider.send('childstate_getStorageEntries', params, !!hash).then((values)=>_.zip(keys, values)).catch(retryOnError);
        }
        // main storage key, use state_getStorageAt
        const params = [
            keys
        ];
        if (hash) params.push(hash);
        return this.#provider.send('state_queryStorageAt', params, !!hash).then((result)=>result[0]?.['changes'] || []).catch(retryOnError);
    }
    /** Upstream `state_getReadProof`. Throws if upstream doesn't expose it. */ async getReadProof(keys, hash) {
        const params = hash ? [
            keys,
            hash
        ] : [
            keys
        ];
        return this.send('state_getReadProof', params, !!hash);
    }
    async subscribeRemoteNewHeads(cb) {
        if (!this.#provider.hasSubscriptions) {
            throw new Error('subscribeRemoteNewHeads only works with subscriptions');
        }
        return this.#provider.subscribe('chain_newHead', 'chain_subscribeNewHeads', [], cb);
    }
    async subscribeRemoteFinalizedHeads(cb) {
        if (!this.#provider.hasSubscriptions) {
            throw new Error('subscribeRemoteFinalizedHeads only works with subscriptions');
        }
        return this.#provider.subscribe('chain_finalizedHead', 'chain_subscribeFinalizedHeads', [], cb);
    }
}
