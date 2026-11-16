import type { HexString } from '@polkadot/util/types';
import type { Handler } from '../shared.js';
/**
 * Submit the extrinsic to the transaction pool
 *
 * @param context
 * @param params - [`extrinsic`]
 *
 * @return operation id
 */
export declare const transaction_v1_broadcast: Handler<[HexString], string | null>;
/**
 * Stop broadcasting the transaction to other nodes.
 *
 */
export declare const transaction_v1_stop: Handler<[string], null>;
