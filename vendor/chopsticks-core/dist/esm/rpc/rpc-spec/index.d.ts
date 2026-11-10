import * as ArchiveV1RPC from './archive_v1.js';
import * as ChainHeadV1RPC from './chainHead_v1.js';
import * as ChainSpecV1RPC from './chainSpec_v1.js';
import * as TransactionV1RPC from './transaction_v1.js';
export { ChainHeadV1RPC, TransactionV1RPC, ChainSpecV1RPC };
declare const handlers: {
    chainSpec_v1_chainName: import("../shared.js").Handler<[], string>;
    chainSpec_v1_genesisHash: import("../shared.js").Handler<[], import("@polkadot/util/types").HexString>;
    chainSpec_v1_properties: import("../shared.js").Handler<[], import("../../index.js").ChainProperties>;
    transaction_v1_broadcast: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], string | null>;
    transaction_v1_stop: import("../shared.js").Handler<[string], null>;
    chainHead_v1_follow: import("../shared.js").Handler<[boolean], string>;
    chainHead_v1_unfollow: import("../shared.js").Handler<[string], null>;
    chainHead_v1_header: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString | null>;
    chainHead_v1_call: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString, string, import("@polkadot/util/types").HexString], {
        result: "started";
        operationId: string;
    }>;
    chainHead_v1_storage: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString, ChainHeadV1RPC.StorageItemRequest[], import("@polkadot/util/types").HexString | null], ChainHeadV1RPC.StorageStarted>;
    chainHead_v1_body: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString], {
        result: "started";
        operationId: string;
    } | ChainHeadV1RPC.LimitReached>;
    chainHead_v1_continue: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString], null>;
    chainHead_v1_stopOperation: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString], null>;
    chainHead_v1_unpin: import("../shared.js").Handler<[string, import("@polkadot/util/types").HexString | import("@polkadot/util/types").HexString[]], null>;
    archive_v1_body: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString[] | null>;
    archive_v1_call: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, string, import("@polkadot/util/types").HexString], ArchiveV1RPC.CallResult | null>;
    archive_v1_finalizedHeight: import("../shared.js").Handler<undefined, number>;
    archive_v1_genesisHash: import("../shared.js").Handler<undefined, import("@polkadot/util/types").HexString>;
    archive_v1_hashByHeight: import("../shared.js").Handler<[number], import("@polkadot/util/types").HexString[]>;
    archive_v1_header: import("../shared.js").Handler<[import("@polkadot/util/types").HexString], import("@polkadot/util/types").HexString | null>;
    archive_v1_storage: import("../shared.js").Handler<[import("@polkadot/util/types").HexString, ChainHeadV1RPC.StorageItemRequest[], import("@polkadot/util/types").HexString | null], string>;
    archive_v1_stopStorage: import("../shared.js").Handler<[string], null>;
};
export default handlers;
