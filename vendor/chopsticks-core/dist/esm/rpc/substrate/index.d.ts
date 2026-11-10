import * as ArchiveRPC from './archive.js';
import * as AuthorRPC from './author.js';
import * as ChainRPC from './chain.js';
import * as PaymentRPC from './payment.js';
import * as StateRPC from './state.js';
import * as SystemRPC from './system.js';
export { ArchiveRPC };
export { AuthorRPC };
export { ChainRPC };
export { PaymentRPC };
export { StateRPC };
export { SystemRPC };
declare const handlers: {
    system_localPeerId: () => Promise<string>;
    system_nodeRoles: () => Promise<string[]>;
    system_localListenAddresses: () => Promise<never[]>;
    system_chain: import("../shared.js").Handler<void, string>;
    system_properties: import("../shared.js").Handler<void, import("../../index.js").ChainProperties>;
    system_name: import("../shared.js").Handler<void, string>;
    system_version: import("../shared.js").Handler<void, string>;
    system_chainType: import("../shared.js").Handler<void, string>;
    system_health: () => Promise<{
        peers: number;
        isSyncing: boolean;
        shouldHavePeers: boolean;
    }>;
    system_dryRun: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], string>;
    system_accountNextIndex: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], number>;
    state_getRuntimeVersion: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("../../index.js").RuntimeVersion | null>;
    state_getMetadata: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString | null>;
    state_getStorage: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], string | null>;
    state_getKeysPaged: import("../shared.js").Handler<[string, number, string, import("@polkadot/util/types").HexString], string[] | undefined>;
    state_queryStorageAt: import("../shared.js").Handler<[string[], import("@polkadot/util/types").HexString], [] | [{
        block: import("@polkadot/util/types").HexString;
        changes: [string, string | null][];
    }]>;
    state_call: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString>;
    state_getReadProof: import("../shared.js").Handler<[import("@polkadot/util/types").HexString[], import("@polkadot/util/types").HexString | undefined], {
        at: import("@polkadot/util/types").HexString;
        proof: import("@polkadot/util/types").HexString[];
    }>;
    state_subscribeRuntimeVersion: import("../shared.js").Handler<[], string>;
    state_unsubscribeRuntimeVersion: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], void>;
    state_subscribeStorage: import("../shared.js").Handler<[string[]], string>;
    state_unsubscribeStorage: import("../shared.js").Handler<[string], void>;
    childstate_getStorage: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], string | null>;
    childstate_getKeysPaged: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString, number, import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString[] | undefined>;
    state_getStorageAt: import("../shared.js").Handler<[`0x${string}`, `0x${string}`], string | null>;
    payment_queryFeeDetails: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString>;
    payment_queryInfo: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString>;
    chain_getBlockHash: import("../shared.js").Handler<[number | import("@polkadot/util/types").HexString | number[] | import("@polkadot/util/types").HexString[] | null], import("@polkadot/util/types").HexString | (import("@polkadot/util/types").HexString | null)[] | null>;
    chain_getHeader: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("../../index.js").Header>;
    chain_getBlock: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], {
        block: {
            header: import("../../index.js").Header;
            extrinsics: import("@polkadot/util/types").HexString[];
        };
        justifications: null;
    }>;
    chain_getFinalizedHead: import("../shared.js").Handler<void, import("@polkadot/util/types").HexString>;
    chain_subscribeNewHead: import("../shared.js").Handler<void, string>;
    chain_subscribeFinalizedHeads: import("../shared.js").Handler<void, string>;
    chain_unsubscribeNewHead: import("../shared.js").Handler<[string], void>;
    chain_getHead: import("../shared.js").Handler<[number | `0x${string}` | `0x${string}`[] | number[] | null], `0x${string}` | (`0x${string}` | null)[] | null>;
    chain_subscribeNewHeads: import("../shared.js").Handler<void, string>;
    chain_unsubscribeNewHeads: import("../shared.js").Handler<[string], void>;
    chain_unsubscribeFinalizedHeads: import("../shared.js").Handler<[string], void>;
    author_submitExtrinsic: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString>;
    author_submitAndWatchExtrinsic: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], string>;
    author_unwatchExtrinsic: import("../shared.js").Handler<[string], void>;
    author_pendingExtrinsics: import("../shared.js").Handler<void, import("@polkadot/util/types").HexString[]>;
    archive_unstable_body: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString[]>;
    archive_unstable_call: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, string, import("@polkadot/util/types").HexString], {
        success: boolean;
        value: `0x${string}`;
    }>;
    archive_unstable_hashByHeight: import("../shared.js").Handler<[number | `0x${string}` | `0x${string}`[] | number[] | null], `0x${string}` | (`0x${string}` | null)[] | null>;
};
export default handlers;
