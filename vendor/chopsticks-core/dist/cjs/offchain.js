"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "OffchainWorker", {
    enumerable: true,
    get: function() {
        return OffchainWorker;
    }
});
const _utilcrypto = require("@polkadot/util-crypto");
const _rxjs = require("rxjs");
const _logger = require("./logger.js");
function _define_property(obj, key, value) {
    if (key in obj) {
        Object.defineProperty(obj, key, {
            value: value,
            enumerable: true,
            configurable: true,
            writable: true
        });
    } else {
        obj[key] = value;
    }
    return obj;
}
const logger = _logger.defaultLogger.child({
    name: 'offchain'
});
class OffchainWorker {
    get(key) {
        return this.offchainStorage.get(key);
    }
    set(key, value) {
        this.offchainStorage.set(key, value);
    }
    async run(block) {
        logger.info({
            number: block.number,
            hash: block.hash
        }, `Run Offchain Worker for block #${block.number.toLocaleString()}`);
        const header = await block.header;
        await block.call('OffchainWorkerApi_offchain_worker', [
            header.toHex()
        ]);
        logger.info(`Offchain Worker complete for block #${block.number.toLocaleString()}`);
        const txs = this.pendingExtrinsics.splice(0);
        if (txs.length > 0) {
            _rxjs.queueScheduler.schedule(async (transactions)=>{
                await block.chain.txPool.buildBlock({
                    transactions
                });
            }, 100, txs);
        }
    }
    async pushExtrinsic(block, extrinsic) {
        const validity = await block.chain.validateExtrinsic(extrinsic, '0x01');
        if (validity.isOk) {
            this.pendingExtrinsics.push(extrinsic);
            return (0, _utilcrypto.blake2AsHex)(extrinsic, 256);
        }
        throw validity.asErr;
    }
    constructor(){
        _define_property(this, "pendingExtrinsics", []);
        _define_property(this, "offchainStorage", new Map());
    }
}
