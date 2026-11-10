"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "timeTravel", {
    enumerable: true,
    get: function() {
        return timeTravel;
    }
});
const _util = require("@polkadot/util");
const _index = require("./index.js");
const _setstorage = require("./set-storage.js");
const timeTravel = async (chain, timestamp)=>{
    const meta = await chain.head.meta;
    const slotDuration = await (0, _index.getSlotDuration)(chain.head);
    const newSlot = Math.floor(timestamp / slotDuration);
    // new timestamp
    const storage = [
        [
            (0, _index.compactHex)(meta.query.timestamp.now()),
            (0, _util.u8aToHex)(meta.registry.createType('u64', timestamp).toU8a())
        ]
    ];
    if (meta.consts.babe) {
        // new slot
        storage.push([
            (0, _index.compactHex)(meta.query.babe.currentSlot()),
            (0, _util.u8aToHex)(meta.registry.createType('Slot', newSlot).toU8a())
        ]);
        // new epoch
        const epochDuration = meta.consts.babe.epochDuration.toNumber();
        const newEpoch = Math.floor(timestamp / epochDuration);
        storage.push([
            (0, _index.compactHex)(meta.query.babe.epochIndex()),
            (0, _util.u8aToHex)(meta.registry.createType('u64', newEpoch).toU8a())
        ]);
    } else if (meta.query.aura) {
        // new slot
        storage.push([
            (0, _index.compactHex)(meta.query.aura.currentSlot()),
            (0, _util.u8aToHex)(meta.registry.createType('Slot', newSlot).toU8a())
        ]);
    }
    await (0, _setstorage.setStorage)(chain, storage);
};
