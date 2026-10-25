"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "SetTimestamp", {
    enumerable: true,
    get: function() {
        return SetTimestamp;
    }
});
const _types = require("@polkadot/types");
const _index = require("../../utils/index.js");
class SetTimestamp {
    async createInherents(newBlock) {
        const parent = await newBlock.parentBlock;
        if (!parent) throw new Error('parent block not found');
        const meta = await parent.meta;
        const slotDuration = await (0, _index.getSlotDuration)(newBlock);
        const currentTimestamp = await (0, _index.getCurrentTimestamp)(parent);
        return [
            new _types.GenericExtrinsic(meta.registry, meta.tx.timestamp.set(currentTimestamp + BigInt(slotDuration))).toHex()
        ];
    }
}
