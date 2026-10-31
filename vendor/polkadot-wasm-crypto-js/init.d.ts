import type { createWasm } from '@polkadot/wasm-crypto-wasm';
export type CryptoModule = ReturnType<typeof createWasm>;
export type CryptoFactory = () => CryptoModule | null | Promise<CryptoModule | null>;
export interface CryptoBridge {
    wasm: CryptoModule | null;
    type: 'wasm' | 'none';
    error: string | null;
    init(factory?: CryptoFactory): Promise<CryptoModule | null>;
}
export declare const bridge: CryptoBridge;
export declare function initBridge(factory?: CryptoFactory): Promise<CryptoModule | null>;
