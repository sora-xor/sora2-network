declare const handlers: {
    dev_getReadProof: import("../shared.js").Handler<[`0x${string}`[], `0x${string}` | undefined], {
        at: import("@polkadot/util/types").HexString;
        proof: import("@polkadot/util/types").HexString[];
        stateRoot: import("@polkadot/util/types").HexString;
    }>;
    dev_newBlock: (context: import("../shared.js").Context, [params]: [import("./new-block.js").NewBlockParams]) => Promise<`0x${string}`>;
    dev_setBlockBuildMode: (context: import("../shared.js").Context, [mode]: [import("../../index.js").BuildBlockMode]) => Promise<void>;
    dev_setHead: (context: import("../shared.js").Context, [params]: [number | `0x${string}`]) => Promise<`0x${string}`>;
    dev_setRuntimeLogLevel: (context: import("../shared.js").Context, [runtimeLogLevel]: [number]) => Promise<void>;
    dev_setStorage: (context: import("../shared.js").Context, params: [import("../../index.js").StorageValues, import("@polkadot/util/types").HexString?]) => Promise<`0x${string}`>;
    dev_timeTravel: (context: import("../shared.js").Context, [date]: [string | number]) => Promise<number>;
};
export default handlers;
export * from './get-read-proof.js';
export * from './new-block.js';
export * from './set-block-build-mode.js';
export * from './set-head.js';
export * from './set-runtime-log-level.js';
export * from './set-storage.js';
export * from './time-travel.js';
