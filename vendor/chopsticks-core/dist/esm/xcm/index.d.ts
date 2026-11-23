import type { Blockchain } from '../blockchain/index.js';
export declare const xcmLogger: import("pino").Logger<never, boolean>;
export declare const connectVertical: (relaychain: Blockchain, parachain: Blockchain) => Promise<void>;
export declare const connectParachains: (parachains: Blockchain[], disableAutoHrmp?: boolean) => Promise<void>;
