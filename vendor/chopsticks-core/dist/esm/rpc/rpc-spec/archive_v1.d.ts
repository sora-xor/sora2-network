import type { HexString } from '@polkadot/util/types';
import { type Handler } from '../shared.js';
import type { StorageItemRequest } from './chainHead_v1.js';
/**
 * Retrieve the body of a specific block
 *
 * @param context
 * @param params - [`hash`]
 *
 * @return An array of the SCALE-encoded transactions of a block, or `null` if the block is not found.
 */
export declare const archive_v1_body: Handler<[HexString], HexString[] | null>;
export type CallResult = {
    success: true;
    value: HexString;
} | {
    success: false;
    error: any;
};
export declare const archive_v1_call: Handler<[HexString, string, HexString], CallResult | null>;
/**
 * Retrieve the height of the finalized block.
 *
 * @param context
 *
 * @return The `number` of the height of the head (a.k.a. finalized) block.
 */
export declare const archive_v1_finalizedHeight: Handler<undefined, number>;
/**
 * Retrieve the genesis hash
 *
 * @param context
 *
 * @return An {@link HexString} with the hash of the genesis block.
 */
export declare const archive_v1_genesisHash: Handler<undefined, HexString>;
/**
 * Retrieve the hash of a specific height
 *
 * @param context
 * @param params - [`height`]
 *
 * @return An array of {@link HexString} with the hashes of the blocks associated to the
 * given height.
 */
export declare const archive_v1_hashByHeight: Handler<[number], HexString[]>;
/**
 * Retrieve the header for a specific block
 *
 * @param context
 * @param params - [`hash`]
 *
 * @return SCALE-encoded header, or `null` if the block is not found.
 */
export declare const archive_v1_header: Handler<[HexString], HexString | null>;
/**
 * Query the storage for a given block
 *
 * @param context
 * @param params - [`hash`, `items`, `childTrie`]
 *
 * @return the operationId to capture the notifications where to receive the result
 *
 * The query type `closestDescendantMerkleValue` is not up to spec.
 * According to the spec, the result should be the Merkle value of the key or
 * the closest descendant of the key.
 * As chopsticks doesn't have direct access to the Merkle tree, it will return
 * a string that will change every time that one of the descendant changes, but
 * it won't be the actual Merkle value.
 * This should be enough for applications that don't rely on the actual Merkle
 * value, but just use it to detect for storage changes.
 */
export declare const archive_v1_storage: Handler<[HexString, StorageItemRequest[], HexString | null], string>;
export declare const archive_v1_stopStorage: Handler<[string], null>;
