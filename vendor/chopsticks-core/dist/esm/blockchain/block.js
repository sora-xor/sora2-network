import { Metadata } from '@polkadot/types';
import { expandMetadata } from '@polkadot/types/metadata';
import { hexToU8a, stringToHex } from '@polkadot/util';
import { compactHex } from '../utils/index.js';
import { getRuntimeVersion, runTask, taskHandler } from '../wasm-executor/index.js';
import { RemoteStorageLayer, StorageLayer, StorageValueKind } from './storage-layer.js';
/**
 * Block class.
 *
 * @example Instantiate a block
 *
 * ```ts
 * const block = new Block(chain, number, hash)
 * ```
 *
 * @example Get storage
 *
 * ```ts
 * const block = await chain.getBlock('0x...')
 * block.storage()
 * ```
 */ export class Block {
    number;
    hash;
    #chain;
    #header;
    #parentBlock;
    #extrinsics;
    #wasm;
    #runtimeVersion;
    #metadata;
    #registry;
    #meta;
    #baseStorage;
    #storages;
    constructor(chain, number, hash, parentBlock, block){
        this.number = number;
        this.hash = hash;
        this.#chain = chain;
        this.#parentBlock = parentBlock ? new WeakRef(parentBlock) : undefined;
        this.#header = block?.header;
        this.#extrinsics = block?.extrinsics;
        this.#baseStorage = block?.storage ?? new RemoteStorageLayer(chain.api, hash, chain.db);
        this.#storages = [];
        this.#runtimeVersion = parentBlock?.runtimeVersion;
        this.#metadata = parentBlock?.metadata;
        this.#registry = parentBlock?.registry;
        this.#meta = parentBlock?.meta;
        const storageDiff = block?.storageDiff;
        if (storageDiff) {
            // if code doesn't change then keep parent block's meta
            // otherwise reset meta
            if (storageDiff[stringToHex(':code')]) {
                this.#runtimeVersion = undefined;
                this.#metadata = undefined;
                this.#registry = undefined;
                this.#meta = undefined;
            }
            this.pushStorageLayer().setAll(storageDiff);
        }
    }
    get chain() {
        return this.#chain;
    }
    get header() {
        if (!this.#header) {
            this.#header = Promise.all([
                this.registry,
                this.#chain.api.getHeader(this.hash)
            ]).then(([registry, header])=>registry.createType('Header', header));
        }
        return this.#header;
    }
    get extrinsics() {
        if (!this.#extrinsics) {
            this.#extrinsics = this.#chain.api.getBlock(this.hash).then((b)=>{
                if (!b) {
                    throw new Error(`Block ${this.hash} not found`);
                }
                return b.block.extrinsics;
            });
        }
        return this.#extrinsics;
    }
    get parentBlock() {
        if (this.number === 0) {
            return Promise.resolve(undefined);
        }
        const getBlock = async (header)=>{
            const _header = await header;
            const block = await this.#chain.getBlock(_header.parentHash.toHex());
            if (block) this.#parentBlock = new WeakRef(block);
            return block;
        };
        if (this.#parentBlock instanceof WeakRef) {
            const block = this.#parentBlock.deref();
            if (block) return Promise.resolve(block);
            this.#parentBlock = getBlock(this.header);
        } else if (!this.#parentBlock) {
            this.#parentBlock = getBlock(this.header);
        }
        return this.#parentBlock;
    }
    /**
   * Get the block storage.
   */ get storage() {
        return this.#storages[this.#storages.length - 1] ?? this.#baseStorage;
    }
    /**
   * Get the block storage by key.
   */ async get(key) {
        const val = await this.storage.get(key, true);
        switch(val){
            case StorageValueKind.Deleted:
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
                case StorageValueKind.Deleted:
                    return undefined;
                default:
                    return val;
            }
        });
    }
    async read(type, query, ...args) {
        const key = compactHex(query(...args));
        const value = await this.get(key);
        if (!value) {
            return undefined;
        }
        const registry = await this.registry;
        return registry.createType(type, hexToU8a(value));
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
        const layer = new StorageLayer(this.storage);
        this.#storages.push(layer);
        return layer;
    }
    /**
   * Pop a layer from the storage stack.
   */ popStorageLayer() {
        this.#storages.pop();
    }
    /**
   * Truncate the storage layer stack back to a target depth.
   *
   * Used by snapshot-restore mechanisms to undo accumulated dev.setStorage
   * calls when reverting to a previously captured state, without losing the
   * underlying block.
   */ resetStorageLayers(targetCount) {
        while(this.#storages.length > targetCount)this.#storages.pop();
    }
    /** The current depth of the storage layer stack. */ get storageLayerCount() {
        return this.#storages.length;
    }
    /**
   * Get storage diff.
   */ async storageDiff() {
        const storage = {};
        for (const layer of this.#storages){
            await layer.mergeInto(storage);
        }
        return storage;
    }
    /**
   * Get the wasm string.
   */ get wasm() {
        if (!this.#wasm) {
            this.#wasm = (async ()=>{
                const wasmKey = stringToHex(':code');
                const wasm = await this.get(wasmKey);
                if (!wasm) {
                    throw new Error('No wasm found');
                }
                return wasm;
            })();
        }
        return this.#wasm;
    }
    /**
   * Set the runtime wasm.
   */ setWasm(wasm) {
        const wasmKey = stringToHex(':code');
        this.pushStorageLayer().set(wasmKey, wasm);
        this.#wasm = Promise.resolve(wasm);
        this.#runtimeVersion = undefined;
        this.#registry = undefined;
        this.#meta = undefined;
        this.#metadata = undefined;
    }
    /**
   * Get the type registry.
   * @see https://polkadot.js.org/docs/api/start/types.create#why-create-types
   */ get registry() {
        if (!this.#registry) {
            this.#registry = Promise.all([
                this.metadata,
                this.runtimeVersion
            ]).then(([data, version])=>this.#chain.buildRegistry(data, version));
        }
        return this.#registry;
    }
    get runtimeVersion() {
        if (!this.#runtimeVersion) {
            this.#runtimeVersion = this.wasm.then(getRuntimeVersion);
        }
        return this.#runtimeVersion;
    }
    get metadata() {
        if (!this.#metadata) {
            this.#metadata = this.call('Metadata_metadata', []).then((resp)=>compactHex(hexToU8a(resp.result)));
        }
        return this.#metadata;
    }
    get meta() {
        if (!this.#meta) {
            this.#meta = Promise.all([
                this.registry,
                this.metadata
            ]).then(([registry, metadataStr])=>{
                const metadata = new Metadata(registry, metadataStr);
                return expandMetadata(registry, metadata);
            });
        }
        return this.#meta;
    }
    /**
   * Call a runtime method.
   */ async call(method, args, mockSigantureHostOverride = false) {
        const wasm = await this.wasm;
        const response = await runTask({
            wasm,
            calls: [
                [
                    method,
                    args
                ]
            ],
            mockSignatureHost: this.#chain.mockSignatureHost,
            allowUnresolvedImports: this.#chain.allowUnresolvedImports,
            runtimeLogLevel: this.#chain.runtimeLogLevel
        }, taskHandler(this), mockSigantureHostOverride);
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
}
