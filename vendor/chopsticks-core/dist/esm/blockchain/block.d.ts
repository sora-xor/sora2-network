import { type TypeRegistry } from '@polkadot/types';
import type { Header } from '@polkadot/types/interfaces';
import type { DecoratedMeta } from '@polkadot/types/metadata/decorate/types';
import type { StorageEntry } from '@polkadot/types/primitive/types';
import type { HexString } from '@polkadot/util/types';
import type { RuntimeVersion, TaskCallResponse } from '../wasm-executor/index.js';
import type { Blockchain } from './index.js';
import { StorageLayer, type StorageLayerProvider, type StorageValue } from './storage-layer.js';
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
 */
export declare class Block {
    #private;
    readonly number: number;
    readonly hash: HexString;
    constructor(chain: Blockchain, number: number, hash: HexString, parentBlock?: Block, block?: {
        /** See `@polkadot/types/interfaces` Header */
        header: Header;
        /** Extrinsics */
        extrinsics: HexString[];
        /** Storage provider. Default to {@link RemoteStorageLayer} with {@link Blockchain.api chain.api} as remote. */
        storage?: StorageLayerProvider;
        /** Storage diff to apply. */
        storageDiff?: Record<string, StorageValue | null>;
    });
    get chain(): Blockchain;
    get header(): Header | Promise<Header>;
    get extrinsics(): HexString[] | Promise<HexString[]>;
    get parentBlock(): Promise<Block | undefined>;
    /**
     * Get the block storage.
     */
    get storage(): StorageLayerProvider;
    /**
     * Get the block storage by key.
     */
    get(key: string): Promise<HexString | undefined>;
    /**
     * Get the block storage by key.
     */
    getMany(keys: string[]): Promise<Array<HexString | undefined>>;
    read<T extends string>(type: T, query: StorageEntry, ...args: any[]): Promise<import("@polkadot/types/types").DetectCodec<import("@polkadot/types-codec/types").Codec, T> | undefined>;
    /**
     * Get paged storage keys.
     */
    getKeysPaged(options: {
        prefix?: string;
        startKey?: string;
        pageSize: number;
    }): Promise<string[]>;
    /**
     * Push a layer to the storage stack.
     */
    pushStorageLayer(): StorageLayer;
    /**
     * Pop a layer from the storage stack.
     */
    popStorageLayer(): void;
    /**
     * Truncate the storage layer stack back to a target depth.
     *
     * Used by snapshot-restore mechanisms to undo accumulated dev.setStorage
     * calls when reverting to a previously captured state, without losing the
     * underlying block.
     */
    resetStorageLayers(targetCount: number): void;
    /** The current depth of the storage layer stack. */
    get storageLayerCount(): number;
    /**
     * Get storage diff.
     */
    storageDiff(): Promise<Record<HexString, HexString | null>>;
    /**
     * Get the wasm string.
     */
    get wasm(): Promise<`0x${string}`>;
    /**
     * Set the runtime wasm.
     */
    setWasm(wasm: HexString): void;
    /**
     * Get the type registry.
     * @see https://polkadot.js.org/docs/api/start/types.create#why-create-types
     */
    get registry(): Promise<TypeRegistry>;
    get runtimeVersion(): Promise<RuntimeVersion>;
    get metadata(): Promise<HexString>;
    get meta(): Promise<DecoratedMeta>;
    /**
     * Call a runtime method.
     */
    call(method: string, args: HexString[], mockSigantureHostOverride?: boolean): Promise<TaskCallResponse>;
}
