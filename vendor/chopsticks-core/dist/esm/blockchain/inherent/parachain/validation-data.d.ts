import type { HexString } from '@polkadot/util/types';
import type { Block } from '../../block.js';
import type { BuildBlockParams, DownwardMessage, HorizontalMessage } from '../../txpool.js';
import type { InherentProvider } from '../index.js';
export type ValidationData = {
    downwardMessages?: DownwardMessage[];
    horizontalMessages?: Record<number, HorizontalMessage[]>;
    validationData: {
        relayParentNumber: number;
        relayParentStorageRoot: HexString;
        maxPovSize: number;
    };
    relayChainState: {
        trieNodes: HexString[];
    };
    relayParentDescendants?: any[];
};
export declare class SetValidationData implements InherentProvider {
    createInherents(newBlock: Block, params: BuildBlockParams): Promise<HexString[]>;
}
