import { blake2AsHex } from '@polkadot/util-crypto';
import { queueScheduler } from 'rxjs';
import { defaultLogger } from './logger.js';
const logger = defaultLogger.child({
    name: 'offchain'
});
export class OffchainWorker {
    pendingExtrinsics = [];
    offchainStorage = new Map();
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
            queueScheduler.schedule(async (transactions)=>{
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
            return blake2AsHex(extrinsic, 256);
        }
        throw validity.asErr;
    }
}
