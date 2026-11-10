import type { Blockchain } from '../blockchain/index.js';
export declare const connectHorizontal: (parachains: Record<number, Blockchain>, disableAutoHrmp?: boolean) => Promise<void>;
