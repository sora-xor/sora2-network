import type { HexString } from '@polkadot/util/types';
import { type Handler } from '../shared.js';
/**
 * @param context
 * @param params - [`extrinsic`, `blockhash`]
 *
 * @return result in hash
 */
export declare const payment_queryFeeDetails: Handler<[HexString, HexString], HexString>;
/**
 * @param context
 * @param params - [`extrinsic`, `blockhash`]
 *
 * @return result in hash
 */
export declare const payment_queryInfo: Handler<[HexString, HexString], HexString>;
