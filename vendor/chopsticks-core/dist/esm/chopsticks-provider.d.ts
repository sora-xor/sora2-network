import type { ProviderInterface, ProviderInterfaceCallback, ProviderInterfaceEmitCb, ProviderInterfaceEmitted } from '@polkadot/rpc-provider/types';
import type { Blockchain } from './blockchain/index.js';
import type { Database } from './database.js';
interface SubscriptionHandler {
    callback: ProviderInterfaceCallback;
    type: string;
}
/**
 * Provider for local chopsticks chain
 */
export declare class ChopsticksProvider implements ProviderInterface {
    #private;
    readonly chain: Blockchain;
    constructor(chain: Blockchain);
    static fromEndpoint: (endpoint: string, block?: number | string | null, db?: Database) => Promise<ChopsticksProvider>;
    get hasSubscriptions(): boolean;
    get isClonable(): boolean;
    get isConnected(): boolean;
    get isReady(): Promise<void>;
    clone: () => ChopsticksProvider;
    connect: () => Promise<void>;
    disconnect: () => Promise<void>;
    on: (type: ProviderInterfaceEmitted, sub: ProviderInterfaceEmitCb) => (() => void);
    subscriptionManager: {
        subscribe: (method: string, subid: string, onCancel?: () => void) => (data: any) => void;
        unsubscribe: (subid: string) => void;
    };
    send: <T>(method: string, params: unknown[], _isCacheable?: boolean, subscription?: SubscriptionHandler) => Promise<T>;
    subscribe(type: string, method: string, params: unknown[], callback: ProviderInterfaceCallback): Promise<number | string>;
    unsubscribe(_type: string, method: string, id: number | string): Promise<boolean>;
}
export {};
