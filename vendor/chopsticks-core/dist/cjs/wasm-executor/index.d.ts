import type { JsCallback } from '@acala-network/chopsticks-executor';
import type { HexString } from '@polkadot/util/types';
import * as Comlink from 'comlink';
import _ from 'lodash';
import type { Block } from '../blockchain/block.js';
export type { JsCallback };
export type RuntimeVersion = {
    specName: string;
    implName: string;
    authoringVersion: number;
    specVersion: number;
    implVersion: number;
    apis: [HexString, number][];
    transactionVersion: number;
    stateVersion: number;
};
export type TaskCall = {
    wasm: HexString;
    calls: [string, HexString[]][];
    mockSignatureHost: boolean;
    allowUnresolvedImports: boolean;
    runtimeLogLevel: number;
    storageProofSize?: number;
};
export type RuntimeLog = {
    message: string;
    level?: number;
    target?: string;
};
export type TaskCallResponse = {
    result: HexString;
    storageDiff: [HexString, HexString | null][];
    offchainStorageDiff: [HexString, HexString | null][];
    runtimeLogs: RuntimeLog[];
};
export type TaskResponse = {
    Call: TaskCallResponse;
} | {
    Error: string;
};
export interface WasmExecutor {
    getRuntimeVersion: (code: HexString) => Promise<RuntimeVersion>;
    calculateStateRoot: (entries: [HexString, HexString][], trie_version: number) => Promise<HexString>;
    createProof: (nodes: HexString[], updates: [HexString, HexString | null][]) => Promise<[HexString, HexString[]]>;
    decodeProof: (trieRootHash: HexString, nodes: HexString[]) => Promise<[[HexString, HexString]]>;
    runTask: (task: {
        wasm: HexString;
        calls: [string, HexString[]][];
        mockSignatureHost: number;
        allowUnresolvedImports: boolean;
        runtimeLogLevel: number;
    }, callback?: JsCallback) => Promise<TaskResponse>;
    testing: (callback: JsCallback, key: any) => Promise<any>;
}
export declare const getWorker: () => Promise<{
    remote: Comlink.Remote<WasmExecutor>;
    terminate: () => Promise<void>;
}>;
export declare const getRuntimeVersion: ((code: HexString) => Promise<RuntimeVersion>) & _.MemoizedFunction;
export declare const calculateStateRoot: (entries: [HexString, HexString][], trie_version: number) => Promise<HexString>;
export declare const decodeProof: (trieRootHash: HexString, nodes: HexString[]) => Promise<{
    [key: `0x${string}`]: `0x${string}`;
}>;
export declare const createProof: (nodes: HexString[], updates: [HexString, HexString | null][]) => Promise<{
    trieRootHash: `0x${string}`;
    nodes: `0x${string}`[];
}>;
export declare const runTask: (task: TaskCall, callback?: JsCallback, overrideMockSignatureHost?: boolean) => Promise<{
    Call: TaskCallResponse;
} | {
    Error: string;
}>;
export declare const taskHandler: (block: Block) => JsCallback;
export declare const emptyTaskHandler: {
    getStorage: (_key: HexString) => Promise<never>;
    getNextKey: (_prefix: HexString, _key: HexString) => Promise<never>;
    offchainGetStorage: (_key: HexString) => Promise<never>;
    offchainTimestamp: () => Promise<never>;
    offchainRandomSeed: () => Promise<never>;
    offchainSubmitTransaction: (_tx: HexString) => Promise<never>;
};
export declare const getAuraSlotDuration: ((wasm: HexString) => Promise<number>) & _.MemoizedFunction;
export declare const destroyWorker: () => Promise<void>;
