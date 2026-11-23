import type { HexString } from '@polkadot/util/types';
import type { Block } from '../../blockchain/block.js';
export declare function getDescendantValues(block: Block, params: DescendantValuesParams): Promise<{
    items: Array<{
        key: string;
        value?: HexString;
    }>;
    next: DescendantValuesParams | null;
}>;
export declare const PAGE_SIZE = 1000;
export type DescendantValuesParams = {
    prefix: string;
    startKey: string;
    isDescendantHashes?: boolean;
};
export declare function afterResponse(fn: () => void): Promise<void>;
