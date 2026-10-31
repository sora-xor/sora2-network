import type { HexString } from '@polkadot/util/types';
import type { Header } from '../../index.js';
import { type Handler } from '../shared.js';
/**
 * @param context
 * @param params - [`blockNumber` | `blockNumber[]` | null]
 *
 * @return Block hash | hash[] | null
 */
export declare const chain_getBlockHash: Handler<[
    number | HexString | number[] | HexString[] | null
], HexString | (HexString | null)[] | null>;
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return Header - see `@polkadot/types/interfaces`
 */
export declare const chain_getHeader: Handler<[HexString], Header>;
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return Block header and extrinsics
 */
export declare const chain_getBlock: Handler<[
    HexString
], {
    block: {
        header: Header;
        extrinsics: HexString[];
    };
    justifications: null;
}>;
/**
 * @param context
 *
 * @return head hash
 */
export declare const chain_getFinalizedHead: Handler<void, HexString>;
export declare const chain_subscribeNewHead: Handler<void, string>;
export declare const chain_subscribeFinalizedHeads: Handler<void, string>;
export declare const chain_unsubscribeNewHead: Handler<[string], void>;
export declare const chain_getHead: Handler<[number | `0x${string}` | `0x${string}`[] | number[] | null], `0x${string}` | (`0x${string}` | null)[] | null>;
export declare const chain_subscribeNewHeads: Handler<void, string>;
export declare const chain_unsubscribeNewHeads: Handler<[string], void>;
export declare const chain_unsubscribeFinalizedHeads: Handler<[string], void>;
