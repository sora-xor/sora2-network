"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "connectDownward", {
    enumerable: true,
    get: function() {
        return connectDownward;
    }
});
const _util = require("@polkadot/util");
const _index = require("../utils/index.js");
const _setstorage = require("../utils/set-storage.js");
const _index1 = require("./index.js");
const connectDownward = async (relaychain, parachain)=>{
    const meta = await relaychain.head.meta;
    const paraId = await (0, _index.getParaId)(parachain);
    const downwardMessageQueuesKey = (0, _index.compactHex)(meta.query.dmp.downwardMessageQueues(paraId));
    await relaychain.headState.subscribeStorage([
        downwardMessageQueuesKey
    ], async (head, pairs)=>{
        const value = pairs[0][1];
        if (!value) return;
        const meta = await head.meta;
        const downwardMessageQueuesKey = (0, _index.compactHex)(meta.query.dmp.downwardMessageQueues(paraId));
        // clear relaychain message queue
        await (0, _setstorage.setStorage)(relaychain, [
            [
                downwardMessageQueuesKey,
                null
            ]
        ], head.hash);
        const downwardMessages = meta.registry.createType('Vec<PolkadotCorePrimitivesInboundDownwardMessage>', (0, _util.hexToU8a)(value)).toJSON();
        if (downwardMessages.length === 0) return;
        _index1.xcmLogger.debug({
            downwardMessages
        }, 'downward_message');
        parachain.submitDownwardMessages(downwardMessages);
    });
};
