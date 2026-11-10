import type { HexString } from '@polkadot/util/types';
import type { Block } from './blockchain/block.js';
export declare class OffchainWorker {
    readonly pendingExtrinsics: HexString[];
    readonly offchainStorage: Map<string, string | null | undefined>;
    get(key: string): string | null | undefined;
    set(key: string, value: string | null | undefined): void;
    run(block: Block): Promise<void>;
    pushExtrinsic(block: Block, extrinsic: HexString): Promise<`0x${string}`>;
}
