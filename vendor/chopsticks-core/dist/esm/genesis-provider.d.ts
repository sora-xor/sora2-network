import type { ProviderInterface, ProviderInterfaceCallback, ProviderInterfaceEmitCb, ProviderInterfaceEmitted } from '@polkadot/rpc-provider/types';
import type { HexString } from '@polkadot/util/types';
import { type Genesis } from './schema/index.js';
import { type JsCallback } from './wasm-executor/index.js';
/**
 * Provider to start a chain from genesis
 */
export declare class GenesisProvider implements ProviderInterface {
    #private;
    genesisHeaderLogs: HexString[];
    /**
     * @ignore
     * Create a genesis provider
     *
     * @param genesis - genesis file
     * @requires genesis provider
     */
    constructor(genesis: Genesis);
    get isClonable(): boolean;
    clone: () => GenesisProvider;
    get hasSubscriptions(): boolean;
    get isConnected(): boolean;
    get isReady(): Promise<void>;
    connect: () => Promise<void>;
    disconnect: () => Promise<void>;
    on: (type: ProviderInterfaceEmitted, sub: ProviderInterfaceEmitCb) => (() => void);
    get blockHash(): HexString;
    getHeader: () => Promise<{
        number: HexString;
        stateRoot: `0x${string}`;
        parentHash: string;
        extrinsicsRoot: string;
        digest: {
            logs: `0x${string}`[];
        };
    }>;
    getBlock: () => Promise<{
        block: {
            header: {
                number: HexString;
                stateRoot: `0x${string}`;
                parentHash: string;
                extrinsicsRoot: string;
                digest: {
                    logs: `0x${string}`[];
                };
            };
            extrinsics: never[];
        };
    }>;
    get _jsCallback(): JsCallback;
    send: (method: string, params: unknown[], _isCacheable?: boolean) => Promise<any>;
    subscribe: (_type: string, _method: string, _params: unknown[], _cb: ProviderInterfaceCallback) => Promise<number | string>;
    unsubscribe: (_type: string, _method: string, _id: number | string) => Promise<boolean>;
}
