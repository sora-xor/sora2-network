import { EventEmitter } from 'eventemitter3';
import { defaultLogger, isPrefixedChildKey } from './index.js';
import { genesisSchema } from './schema/index.js';
import { calculateStateRoot, emptyTaskHandler } from './wasm-executor/index.js';
/**
 * Provider to start a chain from genesis
 */ export class GenesisProvider {
    #isConnected = false;
    #eventemitter = new EventEmitter();
    #isReadyPromise;
    #genesis;
    #stateRoot;
    genesisHeaderLogs = [];
    /**
   * @ignore
   * Create a genesis provider
   *
   * @param genesis - genesis file
   * @requires genesis provider
   */ constructor(genesis){
        this.#genesis = genesisSchema.parse(genesis);
        this.#stateRoot = calculateStateRoot(Object.entries(this.#genesis.genesis.raw.top).reduce((accu, item)=>{
            accu.push(item);
            return accu;
        }, []), 1);
        this.#isReadyPromise = new Promise((resolve, reject)=>{
            this.#eventemitter.once('connected', ()=>{
                resolve();
            });
            this.#eventemitter.once('error', reject);
            this.connect();
        });
    }
    get isClonable() {
        return true;
    }
    clone = ()=>{
        return new GenesisProvider(this.#genesis);
    };
    get hasSubscriptions() {
        return false;
    }
    get isConnected() {
        return this.#isConnected;
    }
    get isReady() {
        return this.#isReadyPromise;
    }
    connect = async ()=>{
        this.#isConnected = true;
        this.#eventemitter.emit('connected');
    };
    disconnect = async ()=>{
        this.#isConnected = false;
        this.#eventemitter.emit('disconnected');
    };
    on = (type, sub)=>{
        this.#eventemitter.on(type, sub);
        return ()=>{
            this.#eventemitter.removeListener(type, sub);
        };
    };
    get blockHash() {
        return '0x4545454545454545454545454545454545454545454545454545454545454545';
    }
    getHeader = async ()=>{
        return {
            number: '0x0',
            stateRoot: await this.#stateRoot,
            parentHash: '0x4545454545454545454545454545454545454545454545454545454545454545',
            extrinsicsRoot: '0x03170a2e7597b7b7e3d84c05391d139a62b157e78786d8c082f29dcf4c111314',
            digest: {
                logs: this.genesisHeaderLogs
            }
        };
    };
    getBlock = async ()=>{
        return {
            block: {
                header: await this.getHeader(),
                extrinsics: []
            }
        };
    };
    get _jsCallback() {
        const storage = this.#genesis.genesis.raw.top;
        return {
            ...emptyTaskHandler,
            getStorage: async (key)=>{
                if (isPrefixedChildKey(key)) {
                    defaultLogger.warn({
                        key
                    }, 'genesis child storage not supported');
                    return undefined;
                }
                return storage[key];
            },
            getNextKey: async (prefix, key)=>{
                if (isPrefixedChildKey(key)) {
                    defaultLogger.warn({
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
    send = async (method, params, _isCacheable)=>{
        await this.isReady;
        switch(method){
            case 'system_properties':
                return this.#genesis.properties;
            case 'system_chain':
                return this.#genesis.id;
            case 'system_name':
                return this.#genesis.name;
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
                    return this.#genesis.genesis.raw.top[params[0]];
                }
            case 'state_queryStorageAt':
                {
                    if (params.length < 2) throw Error('invalid params');
                    const [keys, hash] = params;
                    const values = [];
                    for (const key of keys){
                        const storage = this.#genesis.genesis.raw.top[key];
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
    };
    subscribe = async (_type, _method, _params, _cb)=>{
        throw Error('unimplemented');
    };
    unsubscribe = async (_type, _method, _id)=>{
        throw Error('unimplemented');
    };
}
