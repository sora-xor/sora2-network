"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "connectHorizontal", {
    enumerable: true,
    get: function() {
        return connectHorizontal;
    }
});
const _util = require("@polkadot/util");
const _index = require("../utils/index.js");
const _index1 = require("./index.js");
const connectHorizontal = async (parachains, disableAutoHrmp = false)=>{
    for (const [id, chain] of Object.entries(parachains)){
        const meta = await chain.head.meta;
        const hrmpOutboundMessagesKey = (0, _index.compactHex)(meta.query.parachainSystem.hrmpOutboundMessages());
        await chain.headState.subscribeStorage([
            hrmpOutboundMessagesKey
        ], async (head, pairs)=>{
            const value = pairs[0][1];
            if (!value) return;
            const meta = await head.meta;
            const outboundHrmpMessage = meta.registry.createType('Vec<PolkadotCorePrimitivesOutboundHrmpMessage>', (0, _util.hexToU8a)(value)).toJSON();
            _index1.xcmLogger.info({
                outboundHrmpMessage
            }, 'outboundHrmpMessage');
            for (const { recipient, data } of outboundHrmpMessage){
                const receiver = parachains[recipient];
                if (receiver) {
                    receiver.submitHorizontalMessages(Number(id), [
                        {
                            sentAt: head.number,
                            data
                        }
                    ]);
                }
            }
        });
        const hrmpHeads = await chain.head.read('BTreeMap<u32, H256>', meta.query.parachainSystem.lastHrmpMqcHeads);
        if (hrmpHeads && !disableAutoHrmp) {
            const existingChannels = Array.from(hrmpHeads.keys()).map((x)=>x.toNumber());
            for (const paraId of Object.keys(parachains).filter((x)=>x !== id)){
                if (!existingChannels.includes(Number(paraId))) {
                    chain.submitHorizontalMessages(Number(paraId), []);
                }
            }
        }
    }
};
