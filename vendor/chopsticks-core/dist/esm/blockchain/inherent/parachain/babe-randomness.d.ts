import type { HexString } from '@polkadot/util/types';
import type { Block } from '../../block.js';
import type { BuildBlockParams } from '../../txpool.js';
import type { InherentProvider } from '../index.js';
export declare class SetBabeRandomness implements InherentProvider {
    createInherents(newBlock: Block, _params: BuildBlockParams): Promise<HexString[]>;
}
