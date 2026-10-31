import { TypeRegistry } from '@polkadot/types';
import type { ApplyExtrinsicResult } from '@polkadot/types/interfaces';
import type { TransactionValidity } from '@polkadot/types/interfaces/txqueue';
import type { RegisteredTypes } from '@polkadot/types/types';
import type { HexString } from '@polkadot/util/types';
import type { Api } from '../api.js';
import type { Database } from '../database.js';
import { OffchainWorker } from '../offchain.js';
import type { RuntimeVersion } from '../wasm-executor/index.js';
import { Block } from './block.js';
import { HeadState } from './head-state.js';
import type { InherentProvider } from './inherent/index.js';
import { type BuildBlockMode, type BuildBlockParams, type DownwardMessage, type HorizontalMessage, TxPool } from './txpool.js';
export interface Options {
    /** API instance, for getting on-chain data. */
    api: Api;
    /** Build block mode. Default to Batch. */
    buildBlockMode?: BuildBlockMode;
    /** Inherent provider, for creating inherents. */
    inherentProviders: InherentProvider[];
    /** Datasource for caching storage and blocks data. */
    db?: Database;
    /** Used to create the initial head. */
    header: {
        number: number;
        hash: HexString;
    };
    /** Whether to enable mock signature. Any signature starts with 0xdeadbeef and filled by 0xcd is considered valid */
    mockSignatureHost?: boolean;
    /** Whether to allow wasm unresolved imports. */
    allowUnresolvedImports?: boolean;
    /** Wasm runtime log level. */
    runtimeLogLevel?: number;
    /** Polkadot.js custom types registration. */
    registeredTypes: RegisteredTypes;
    /** Whether to enable offchain Worker. */
    offchainWorker?: boolean;
    /** Max memory block count */
    maxMemoryBlockCount?: number;
    /** Whether to process queued messages */
    processQueuedMessages?: boolean;
    /** Whether to save blocks to db */
    saveBlocks?: boolean;
}
/**
 * Local blockchain which provides access to blocks, txpool and methods
 * to manipulate the chain such as build blocks, submit extrinsics, xcm and more!
 *
 * @example
 *
 * ```ts
 * const chain = new Blockchain({
 *  api,
 *  buildBlockMode: BuildBlockMode.Manual,
 *  inherentProviders,
 *  header: {
 *    hash: blockHash,
 *    number: Number(header.number),
 *  },
 *  mockSignatureHost: true,
 *  allowUnresolvedImports: true,
 *  registeredTypes: {},
 * })
 * // build a block
 * chain.newBlock()
 * ```
 */
export declare class Blockchain {
    #private;
    /** API instance, for getting on-chain data. */
    readonly api: Api;
    /** Datasource for caching storage and blocks data. */
    readonly db: Database | undefined;
    /** Enable mock signature. Any signature starts with 0xdeadbeef and filled by 0xcd is considered valid */
    readonly mockSignatureHost: boolean;
    /** Allow wasm unresolved imports. */
    readonly allowUnresolvedImports: boolean;
    /** Polkadot.js custom types registration. */
    readonly registeredTypes: RegisteredTypes;
    /** For subscribing and managing the head state. */
    readonly headState: HeadState;
    readonly offchainWorker: OffchainWorker | undefined;
    readonly processQueuedMessages: boolean;
    readonly saveBlocks: boolean;
    /**
     * @param options - Options for instantiating the blockchain
     */
    constructor({ api, buildBlockMode, inherentProviders, db, header, mockSignatureHost, allowUnresolvedImports, runtimeLogLevel, registeredTypes, offchainWorker, maxMemoryBlockCount, processQueuedMessages, saveBlocks, }: Options);
    get head(): Block;
    get txPool(): TxPool;
    get runtimeLogLevel(): number;
    set runtimeLogLevel(level: number);
    buildRegistry(metadata: HexString, version: RuntimeVersion): Promise<TypeRegistry>;
    saveBlockToDB(block: Block): Promise<void>;
    /**
     * Try to load block from db and register it.
     * If pass in number, get block by number, else get block by hash.
     */
    loadBlockFromDB(hashOrNumber: number | HexString): Promise<Block | undefined>;
    /**
     * Get block by number.
     */
    getBlockAt(number?: number | null): Promise<Block | undefined>;
    /**
     * Get block by hash.
     */
    getBlock(hash?: HexString): Promise<Block | undefined>;
    /**
     * Get all blocks in memory.
     */
    blocksInMemory(): Block[];
    /**
     * Remove block from memory and db.
     */
    unregisterBlock(block: Block): Promise<void>;
    onNewBlock(block: Block): Promise<void>;
    /**
     * Set block as head.
     */
    setHead(block: Block): Promise<void>;
    /**
     * Submit extrinsic to txpool.
     */
    submitExtrinsic(extrinsic: HexString): Promise<HexString>;
    /**
     * Validate extrinsic by calling `TaggedTransactionQueue_validate_transaction`.
     */
    validateExtrinsic(extrinsic: HexString, source?: '0x00' | '0x01' | '0x02' /** External */): Promise<TransactionValidity>;
    submitUpwardMessages(id: number, ump: HexString[]): void;
    submitDownwardMessages(dmp: DownwardMessage[]): void;
    submitHorizontalMessages(id: number, hrmp: HorizontalMessage[]): void;
    /**
     * Build a new block with optional params. Use this when you don't have all the {@link BuildBlockParams}
     */
    newBlock(params?: Partial<BuildBlockParams>): Promise<Block>;
    /**
     * Build a new block with {@link BuildBlockParams}.
     */
    newBlockWithParams(params: BuildBlockParams): Promise<Block>;
    /**
     * Get the upcoming blocks.
     */
    upcomingBlocks(): Promise<number>;
    /**
     * Dry run extrinsic in block `at`.
     */
    dryRunExtrinsic(extrinsic: HexString | {
        call: HexString;
        address: string;
    }, at?: HexString): Promise<{
        outcome: ApplyExtrinsicResult;
        storageDiff: [HexString, HexString | null][];
    }>;
    /**
     * Dry-run a batch of extrinsics in block `at`, amortizing block initialization
     * across the whole batch. Each extrinsic is applied independently — its storage
     * changes do not accumulate into subsequent extrinsics.
     */
    dryRunExtrinsicsAmortized(extrinsics: HexString[], at?: HexString): Promise<{
        outcome: ApplyExtrinsicResult;
        storageDiff: [HexString, HexString | null][];
    }[]>;
    /**
     * Dry run hrmp messages in block `at`.
     * Return the storage diff.
     */
    dryRunHrmp(hrmp: Record<number, HorizontalMessage[]>, at?: HexString): Promise<[HexString, HexString | null][]>;
    /**
     * Dry run dmp messages in block `at`.
     * Return the storage diff.
     */
    dryRunDmp(dmp: DownwardMessage[], at?: HexString): Promise<[HexString, HexString | null][]>;
    /**
     * Dry run ump messages in block `at`.
     * Return the storage diff.
     */
    dryRunUmp(ump: Record<number, HexString[]>, at?: HexString): Promise<[HexString, HexString | null][]>;
    /**
     * Get inherents of head.
     */
    getInherents(): InherentProvider[];
    /**
     * Close the db and disconnect api.
     */
    close(): Promise<void>;
}
