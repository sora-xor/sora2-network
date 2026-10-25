import type { Config, SubstrateConnect } from './ScInterface.js';
import type { ProviderInterface, ProviderInterfaceCallback, ProviderInterfaceEmitCb, ProviderInterfaceEmitted } from '../types.js';
import { healthChecker } from './Health.js';
export declare class ScProvider implements ProviderInterface {
    #private;
    constructor(Sc: SubstrateConnect, spec: string, sharedSandbox?: ScProvider);
    get hasSubscriptions(): boolean;
    get isClonable(): boolean;
    get isConnected(): boolean;
    clone(): ProviderInterface;
    connect(config?: Config, checkerFactory?: typeof healthChecker): Promise<void>;
    disconnect(): Promise<void>;
    on(type: ProviderInterfaceEmitted, sub: ProviderInterfaceEmitCb): () => void;
    send<T = any>(method: string, params: unknown[]): Promise<T>;
    subscribe(type: string, method: string, params: any[], callback: ProviderInterfaceCallback): Promise<number | string>;
    unsubscribe(type: string, method: string, id: number | string): Promise<boolean>;
}
export {};
