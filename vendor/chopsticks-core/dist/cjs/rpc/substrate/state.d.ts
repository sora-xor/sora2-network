import type { HexString } from '@polkadot/util/types';
import type { RuntimeVersion } from '../../wasm-executor/index.js';
import { type Handler } from '../shared.js';
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return runtime version
 */
export declare const state_getRuntimeVersion: Handler<[HexString], RuntimeVersion | null>;
/**
 * @param context
 * @param params - [`blockhash`]
 *
 * @return metadata
 */
export declare const state_getMetadata: Handler<[HexString], HexString | null>;
/**
 * @param context
 * @param params - [`key`, `blockhash`]
 *
 * @return storage value
 */
export declare const state_getStorage: Handler<[HexString, HexString], string | null>;
/**
 * @param context
 * @param params - [`prefix`, `pageSize`, `startKey`, `blockhash`]
 *
 * @return paged keys
 */
export declare const state_getKeysPaged: Handler<[string, number, string, HexString], string[] | undefined>;
/**
 * @param context
 * @param params - [`keys`, `blockhash`]
 *
 * @return storage values
 */
export declare const state_queryStorageAt: Handler<[
    string[],
    HexString
], [] | [
    {
        block: HexString;
        changes: [string, string | null][];
    }
]>;
/**
 * @param context
 * @param params - [`method`, `data`, `blockhash`]
 *
 * @return result in hash
 */
export declare const state_call: Handler<[HexString, HexString, HexString], HexString>;
/**
 * @param context
 * @param params - [`keys`, `blockhash?`]
 *
 * @return `{ at, proof }` — proof is a list of SCALE-encoded trie nodes.
 *
 * Spec-compatible response shape. The composed proof's root diverges from
 * `chain_getHeader(at).state_root` once local overrides are applied (see `buildReadProof`);
 * callers that need the recomputed root should use chopsticks's `dev_getReadProof`.
 * Child-storage keys are rejected — use `state_getChildReadProof`.
 */
export declare const state_getReadProof: Handler<[
    HexString[],
    HexString | undefined
], {
    at: HexString;
    proof: HexString[];
}>;
/**
 * @return subscription id
 */
export declare const state_subscribeRuntimeVersion: Handler<[], string>;
/**
 * @param context
 * @param params - [`subid`]
 * @param subscriptionManager
 */
export declare const state_unsubscribeRuntimeVersion: Handler<[HexString], void>;
/**
 * @param context
 * @param params - [`keys`]
 * @param subscriptionManager
 *
 * @return subscription id
 */
export declare const state_subscribeStorage: Handler<[string[]], string>;
/**
 * @param context
 * @param params - [`subid`]
 * @param subscriptionManager
 */
export declare const state_unsubscribeStorage: Handler<[string], void>;
/**
 * @param context
 * @param params - [`child`, `key`, `blockhash`]
 *
 * @return storage valuse
 */
export declare const childstate_getStorage: Handler<[HexString, HexString, HexString], string | null>;
/**
 * @param context
 * @param params - [`child`, `prefix`, `pageSize`, `startKey`, `blockhash`]
 *
 * @return paged keys
 */
export declare const childstate_getKeysPaged: Handler<[
    HexString,
    HexString,
    number,
    HexString,
    HexString
], HexString[] | undefined>;
export declare const state_getStorageAt: Handler<[`0x${string}`, `0x${string}`], string | null>;
