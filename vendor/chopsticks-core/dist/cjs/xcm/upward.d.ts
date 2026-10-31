import type { Blockchain } from '../blockchain/index.js';
/** Filter out UMP signals, keeping only XCM messages before the empty separator. */
export declare function filterXcmMessages<T extends {
    length: number;
}>(messages: T[]): T[];
export declare const connectUpward: (parachain: Blockchain, relaychain: Blockchain) => Promise<void>;
