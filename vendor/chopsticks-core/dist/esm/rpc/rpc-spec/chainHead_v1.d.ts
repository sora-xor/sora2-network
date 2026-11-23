import type { HexString } from '@polkadot/util/types';
import { type Handler } from '../shared.js';
/**
 * Start a chainHead follow subscription
 *
 * @param context
 * @param params - [`withRuntime`]
 * @param subscriptionManager
 *
 * @return subscription id
 */
export declare const chainHead_v1_follow: Handler<[boolean], string>;
/**
 * Stop a chainHead follow subscription
 *
 * @param context
 * @param params - [`followSubscription`]
 * @param subscriptionManager
 */
export declare const chainHead_v1_unfollow: Handler<[string], null>;
/**
 * Retrieve the header for a specific block
 *
 * @param context
 * @param params - [`followSubscription`, `hash`]
 *
 * @return SCALE-encoded header, or null if the block is not found.
 */
export declare const chainHead_v1_header: Handler<[string, HexString], HexString | null>;
type OperationStarted = {
    result: 'started';
    operationId: string;
};
/**
 * Perform a runtime call for a block
 *
 * @param context
 * @param params - [`followSubscription`, `hash`, `function`, `callParameters`]
 *
 * @return OperationStarted event with operationId to receive the result on the follow subscription
 */
export declare const chainHead_v1_call: Handler<[string, HexString, string, HexString], OperationStarted>;
export type StorageStarted = OperationStarted & {
    discardedItems: number;
};
export interface StorageItemRequest {
    key: HexString;
    type: 'value' | 'hash' | 'closestDescendantMerkleValue' | 'descendantsValues' | 'descendantsHashes';
}
/**
 * Query the storage for a given block
 *
 * @param context
 * @param params - [`followSubscription`, `hash`, `items`, `childTrie`]
 *
 * @return OperationStarted event with operationId to receive the result on the follow subscription
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
export declare const chainHead_v1_storage: Handler<[
    string,
    HexString,
    StorageItemRequest[],
    HexString | null
], StorageStarted>;
export type LimitReached = {
    result: 'limitReached';
};
/**
 * Retrieve the body of a specific block
 *
 * @param context
 * @param params - [`followSubscription`, `hash`]
 *
 * @return OperationStarted event with operationId to receive the result on the follow subscription
 */
export declare const chainHead_v1_body: Handler<[string, HexString], OperationStarted | LimitReached>;
/**
 * Resume an operation paused through `operationWaitingForContinue`
 *
 * @param context
 * @param params - [`followSubscription`, `operationId`]
 */
export declare const chainHead_v1_continue: Handler<[string, HexString], null>;
export declare const chainHead_v1_stopOperation: Handler<[string, HexString], null>;
export declare const chainHead_v1_unpin: Handler<[string, HexString | HexString[]], null>;
export {};
