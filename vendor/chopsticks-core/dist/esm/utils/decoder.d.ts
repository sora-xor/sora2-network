import '@polkadot/types-codec';
import type { StorageKey } from '@polkadot/types';
import type { DecoratedMeta } from '@polkadot/types/metadata/decorate/types';
import type { StorageEntry } from '@polkadot/types/primitive/types';
import type { HexString } from '@polkadot/util/types';
import type { Block } from '../blockchain/block.js';
export declare const decodeKey: (meta: DecoratedMeta, key: HexString) => {
    storage?: StorageEntry;
    decodedKey?: StorageKey;
};
export declare const decodeKeyValue: (meta: DecoratedMeta, key: HexString, value?: HexString | null, toHuman?: boolean) => {
    section: string;
    method: string;
    key: any[];
    value: import("@polkadot/types-codec/types").AnyJson;
} | undefined;
export declare const toStorageObject: (decoded: ReturnType<typeof decodeKeyValue>) => {
    [x: string]: {
        [x: string]: import("@polkadot/types-codec/types").AnyJson;
    };
} | undefined;
/**
 * Decode block storage diff
 * @param block Block to compare storage diff
 * @param diff Storage diff
 * @returns decoded old state and new state
 */
export declare const decodeBlockStorageDiff: (block: Block, diff: [HexString, HexString | null][]) => Promise<{}[]>;
