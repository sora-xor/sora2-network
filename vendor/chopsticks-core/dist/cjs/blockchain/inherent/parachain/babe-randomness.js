"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "SetBabeRandomness", {
    enumerable: true,
    get: function() {
        return SetBabeRandomness;
    }
});
const _types = require("@polkadot/types");
class SetBabeRandomness {
    async createInherents(newBlock, _params) {
        const parent = await newBlock.parentBlock;
        if (!parent) throw new Error('parent block not found');
        const meta = await parent.meta;
        if (!meta.tx.randomness?.setBabeRandomnessResults) {
            return [];
        }
        return [
            new _types.GenericExtrinsic(meta.registry, meta.tx.randomness.setBabeRandomnessResults()).toHex()
        ];
    }
}
