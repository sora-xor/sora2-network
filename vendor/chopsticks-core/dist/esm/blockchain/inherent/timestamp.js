import { GenericExtrinsic } from '@polkadot/types';
import { getCurrentTimestamp, getSlotDuration } from '../../utils/index.js';
export class SetTimestamp {
    async createInherents(newBlock) {
        const parent = await newBlock.parentBlock;
        if (!parent) throw new Error('parent block not found');
        const meta = await parent.meta;
        const slotDuration = await getSlotDuration(newBlock);
        const currentTimestamp = await getCurrentTimestamp(parent);
        return [
            new GenericExtrinsic(meta.registry, meta.tx.timestamp.set(currentTimestamp + BigInt(slotDuration))).toHex()
        ];
    }
}
