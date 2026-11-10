import { GenericExtrinsic } from '@polkadot/types';
// Support for Moonbeam pallet-randomness mandatory inherent
export class SetBabeRandomness {
    async createInherents(newBlock, _params) {
        const parent = await newBlock.parentBlock;
        if (!parent) throw new Error('parent block not found');
        const meta = await parent.meta;
        if (!meta.tx.randomness?.setBabeRandomnessResults) {
            return [];
        }
        return [
            new GenericExtrinsic(meta.registry, meta.tx.randomness.setBabeRandomnessResults()).toHex()
        ];
    }
}
