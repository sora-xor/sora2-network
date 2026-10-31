import type { HexString } from '@polkadot/util/types';
import type { Blockchain } from '../blockchain/index.js';
type RawStorageValues = [string, string | null][];
type StorageConfig = Record<string, Record<string, any>>;
export type StorageValues = RawStorageValues | StorageConfig;
export declare const setStorage: (chain: Blockchain, storage: StorageValues, blockHash?: HexString) => Promise<HexString>;
export {};
