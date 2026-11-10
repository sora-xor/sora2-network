"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "ProcessPendingRenewals", {
    enumerable: true,
    get: function() {
        return ProcessPendingRenewals;
    }
});
const _types = require("@polkadot/types");
class ProcessPendingRenewals {
    async createInherents(newBlock) {
        const parent = await newBlock.parentBlock;
        if (!parent) throw new Error('parent block not found');
        const meta = await parent.meta;
        if (!meta.tx.dataRenewal?.processPendingRenewals) {
            return [];
        }
        // Bulletin's pallet-bulletin-data-renewal mandatory drain inherent.
        // The pallet's on_finalize asserts PendingAutoRenewals was drained, so
        // the inherent must be included in every block, matching the pallet's
        // unconditional create_inherent.
        const inherent = new _types.GenericExtrinsic(meta.registry, meta.tx.dataRenewal.processPendingRenewals());
        return [
            inherent.toHex()
        ];
    }
}
