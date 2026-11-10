import type { HexString } from '@polkadot/util/types';
import { type Handler } from '../shared.js';
/**
 * @param context
 * @param params - [`extrinsic`]
 *
 * @return Hash
 */
export declare const author_submitExtrinsic: Handler<[HexString], HexString>;
/**
 * @param context
 * @param params - [`extrinsic`]
 * @param subscriptionManager
 *
 * @return subscription id
 */
export declare const author_submitAndWatchExtrinsic: Handler<[HexString], string>;
/**
 * @param _context
 * @param params - [`subid`]
 */
export declare const author_unwatchExtrinsic: Handler<[string], void>;
/**
 * Get pending extrinsics
 *
 * @return Array of pending extrinsics
 */
export declare const author_pendingExtrinsics: Handler<void, HexString[]>;
