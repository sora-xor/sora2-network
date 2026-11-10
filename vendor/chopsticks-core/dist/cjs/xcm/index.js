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
    get connectParachains () {
        return connectParachains;
    },
    get connectVertical () {
        return connectVertical;
    },
    get xcmLogger () {
        return xcmLogger;
    }
});
const _logger = require("../logger.js");
const _index = require("../utils/index.js");
const _downward = require("./downward.js");
const _horizontal = require("./horizontal.js");
const _upward = require("./upward.js");
const xcmLogger = _logger.defaultLogger.child({
    name: 'xcm'
});
const connectVertical = async (relaychain, parachain)=>{
    await (0, _downward.connectDownward)(relaychain, parachain);
    await (0, _upward.connectUpward)(parachain, relaychain);
    xcmLogger.info(`Connected relaychain '${await relaychain.api.getSystemChain()}' with parachain '${await parachain.api.getSystemChain()}'`);
};
const connectParachains = async (parachains, disableAutoHrmp = false)=>{
    const list = {};
    for (const chain of parachains){
        const paraId = await (0, _index.getParaId)(chain);
        list[paraId.toNumber()] = chain;
    }
    await (0, _horizontal.connectHorizontal)(list, disableAutoHrmp);
    xcmLogger.info(`Connected parachains [${Object.keys(list)}]`);
};
