import type { HexString } from '@polkadot/util/types';
import type { ChainProperties } from '../../index.js';
import type { Handler } from '../shared.js';
export declare const system_localPeerId: () => Promise<string>;
export declare const system_nodeRoles: () => Promise<string[]>;
export declare const system_localListenAddresses: () => Promise<never[]>;
export declare const system_chain: Handler<void, string>;
export declare const system_properties: Handler<void, ChainProperties>;
export declare const system_name: Handler<void, string>;
export declare const system_version: Handler<void, string>;
export declare const system_chainType: Handler<void, string>;
export declare const system_health: () => Promise<{
    peers: number;
    isSyncing: boolean;
    shouldHavePeers: boolean;
}>;
/**
 * @param context
 * @param params - [`extrinsic`, `at`]
 *
 * @return ApplyExtrinsicResult (see `@polkadot/types/interfaces`) in hash
 */
export declare const system_dryRun: Handler<[HexString, HexString], string>;
/**
 * @param context
 * @param params - [`address`]
 */
export declare const system_accountNextIndex: Handler<[HexString], number>;
