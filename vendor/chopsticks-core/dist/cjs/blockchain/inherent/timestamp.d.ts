import type { HexString } from '@polkadot/util/types';
import type { Block } from '../block.js';
import type { InherentProvider } from './index.js';
export declare class SetTimestamp implements InherentProvider {
    createInherents(newBlock: Block): Promise<HexString[]>;
}
