import type { HexString } from '@polkadot/util/types';
import type { Block } from '../block.js';
import type { BuildBlockParams } from '../txpool.js';
import { ProcessPendingRenewals } from './data-renewal.js';
import { ParaInherentEnter } from './para-enter.js';
import { SetBabeRandomness } from './parachain/babe-randomness.js';
import { SetNimbusAuthorInherent } from './parachain/nimbus-author-inherent.js';
import { SetValidationData } from './parachain/validation-data.js';
import { SetTimestamp } from './timestamp.js';
import { SetTransactionStorageProof } from './transaction-storage.js';
export interface InherentProvider {
    createInherents(newBlock: Block, params: BuildBlockParams): Promise<HexString[]>;
}
export declare const inherentProviders: (ProcessPendingRenewals | ParaInherentEnter | SetBabeRandomness | SetNimbusAuthorInherent | SetValidationData | SetTimestamp | SetTransactionStorageProof)[];
