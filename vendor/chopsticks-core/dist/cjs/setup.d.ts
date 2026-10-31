import '@polkadot/types-codec';
import type { RegisteredTypes } from '@polkadot/types/types';
import { Api } from './api.js';
import { Blockchain } from './blockchain/index.js';
import type { BuildBlockMode } from './blockchain/txpool.js';
import type { Database } from './database.js';
import type { GenesisProvider } from './genesis-provider.js';
export type SetupOptions = {
    endpoint?: string | string[];
    block?: string | number | null;
    genesis?: GenesisProvider;
    buildBlockMode?: BuildBlockMode;
    db?: Database;
    mockSignatureHost?: boolean;
    allowUnresolvedImports?: boolean;
    runtimeLogLevel?: number;
    registeredTypes?: RegisteredTypes;
    offchainWorker?: boolean;
    maxMemoryBlockCount?: number;
    processQueuedMessages?: boolean;
    saveBlocks?: boolean;
    rpcTimeout?: number;
    hooks?: {
        apiFetching?: () => void;
    };
};
export declare const processOptions: (options: SetupOptions) => Promise<{
    blockHash: string;
    api: Api;
    endpoint?: string | string[];
    block?: string | number | null;
    genesis?: GenesisProvider;
    buildBlockMode?: BuildBlockMode;
    db?: Database;
    mockSignatureHost?: boolean;
    allowUnresolvedImports?: boolean;
    runtimeLogLevel?: number;
    registeredTypes?: RegisteredTypes;
    offchainWorker?: boolean;
    maxMemoryBlockCount?: number;
    processQueuedMessages?: boolean;
    saveBlocks?: boolean;
    rpcTimeout?: number;
    hooks?: {
        apiFetching?: () => void;
    };
}>;
export declare const setup: (options: SetupOptions) => Promise<Blockchain>;
