"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "ParaInherentEnter", {
    enumerable: true,
    get: function() {
        return ParaInherentEnter;
    }
});
const _types = require("@polkadot/types");
class ParaInherentEnter {
    async createInherents(newBlock, _params) {
        const parent = await newBlock.parentBlock;
        if (!parent) throw new Error('parent block not found');
        const meta = await parent.meta;
        if (!meta.tx.paraInherent?.enter) {
            return [];
        }
        if (parent.number === 0) {
            return [
                new _types.GenericExtrinsic(meta.registry, meta.tx.paraInherent.enter({
                    parentHeader: (await parent.header).toJSON()
                })).toHex()
            ];
        }
        const extrinsics = await parent.extrinsics;
        const paraEnterExtrinsic = extrinsics.find((extrinsic)=>{
            const firstArg = meta.registry.createType('GenericExtrinsic', extrinsic)?.args?.[0];
            return firstArg && 'bitfields' in firstArg;
        });
        if (!paraEnterExtrinsic) {
            throw new Error('Missing paraInherent data from block');
        }
        const extrinsic = meta.registry.createType('GenericExtrinsic', paraEnterExtrinsic).args[0].toJSON();
        const parentHeader = (await parent.header).toJSON();
        const newData = {
            ...extrinsic,
            bitfields: [],
            backedCandidates: [],
            parentHeader
        };
        // TODO: fill with data
        return [
            new _types.GenericExtrinsic(meta.registry, meta.tx.paraInherent.enter(newData)).toHex()
        ];
    }
}
