import type { u32 } from '@polkadot/types';
import type { HrmpChannelId } from '@polkadot/types/interfaces';
import type { HexString } from '@polkadot/util/types';
export declare const WELL_KNOWN_KEYS: {
    EPOCH_INDEX: HexString;
    CURRENT_BLOCK_RANDOMNESS: HexString;
    ONE_EPOCH_AGO_RANDOMNESS: HexString;
    TWO_EPOCHS_AGO_RANDOMNESS: HexString;
    CURRENT_SLOT: HexString;
    ACTIVE_CONFIG: HexString;
    AUTHORITIES: HexString;
};
export declare const dmqMqcHead: (paraId: u32) => `0x${string}`;
export declare const upgradeGoAheadSignal: (paraId: u32) => `0x${string}`;
export declare const upgradeRestrictionSignal: (paraId: u32) => `0x${string}`;
export declare const hrmpIngressChannelIndex: (paraId: u32) => `0x${string}`;
export declare const hrmpEgressChannelIndex: (paraId: u32) => `0x${string}`;
export declare const hrmpChannels: (channelId: HrmpChannelId) => `0x${string}`;
export declare const paraHead: (paraId: u32) => `0x${string}`;
