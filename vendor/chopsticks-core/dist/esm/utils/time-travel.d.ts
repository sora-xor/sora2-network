import type { Blockchain } from '../blockchain/index.js';
export declare const timeTravel: (chain: Blockchain, timestamp: number) => Promise<void>;
