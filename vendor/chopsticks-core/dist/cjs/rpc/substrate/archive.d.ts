import type { HexString } from '@polkadot/util/types';
import { type Handler } from '../shared.js';
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return Block extrinsics
 */
export declare const archive_unstable_body: Handler<[HexString], HexString[]>;
/**
 * @param context
 * @param params - [`blockhash`, `method`, `data` ]
 *
 * @return {Object} result The call result.
 * @return {boolean} result.success Whether the call is successful.
 * @return {string} result.value The call result.
 */
export declare const archive_unstable_call: Handler<[
    HexString,
    string,
    HexString
], {
    success: boolean;
    value: `0x${string}`;
}>;
export declare const archive_unstable_hashByHeight: Handler<[number | `0x${string}` | `0x${string}`[] | number[] | null], `0x${string}` | (`0x${string}` | null)[] | null>;
