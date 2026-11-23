import { ProcessPendingRenewals } from './data-renewal.js';
import { ParaInherentEnter } from './para-enter.js';
import { SetBabeRandomness } from './parachain/babe-randomness.js';
import { SetNimbusAuthorInherent } from './parachain/nimbus-author-inherent.js';
import { SetValidationData } from './parachain/validation-data.js';
import { SetTimestamp } from './timestamp.js';
import { SetTransactionStorageProof } from './transaction-storage.js';
export const inherentProviders = [
    new SetTimestamp(),
    new SetValidationData(),
    new ParaInherentEnter(),
    new SetNimbusAuthorInherent(),
    new SetBabeRandomness(),
    new SetTransactionStorageProof(),
    new ProcessPendingRenewals()
];
