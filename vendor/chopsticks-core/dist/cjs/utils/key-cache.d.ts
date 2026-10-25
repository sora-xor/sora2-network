import type { HexString } from '@polkadot/util/types';
export default class KeyCache {
    readonly prefixLength: number;
    constructor(prefixLength: number);
    readonly ranges: Array<{
        prefix: string;
        keys: string[];
    }>;
    feed(keys: HexString[]): void;
    next(startKey: HexString): Promise<HexString | undefined>;
}
