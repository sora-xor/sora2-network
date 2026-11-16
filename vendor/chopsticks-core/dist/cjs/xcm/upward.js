"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: Object.getOwnPropertyDescriptor(all, name).get
    });
}
_export(exports, {
    get connectUpward () {
        return connectUpward;
    },
    get filterXcmMessages () {
        return filterXcmMessages;
    }
});
const _util = require("@polkadot/util");
const _index = require("../utils/index.js");
const _index1 = require("./index.js");
function filterXcmMessages(messages) {
    const separatorIndex = messages.findIndex((m)=>m.length === 0);
    if (separatorIndex === -1) return messages;
    const signalCount = messages.length - separatorIndex - 1;
    _index1.xcmLogger.debug({
        xcmCount: separatorIndex,
        signalCount
    }, 'Filtered UMP signals from upward messages');
    return messages.slice(0, separatorIndex);
}
const connectUpward = async (parachain, relaychain)=>{
    const meta = await parachain.head.meta;
    const paraId = (await (0, _index.getParaId)(parachain)).toNumber();
    const upwardMessagesKey = (0, _index.compactHex)(meta.query.parachainSystem.upwardMessages());
    await parachain.headState.subscribeStorage([
        upwardMessagesKey
    ], async (_head, pairs)=>{
        const value = pairs[0][1];
        if (!value) return;
        const meta = await relaychain.head.meta;
        const upwardMessages = meta.registry.createType('Vec<Bytes>', (0, _util.hexToU8a)(value));
        if (upwardMessages.length === 0) return;
        const xcmMessages = filterXcmMessages(upwardMessages.toArray());
        if (xcmMessages.length === 0) return;
        relaychain.submitUpwardMessages(paraId, xcmMessages.map((x)=>x.toHex()));
    });
};
