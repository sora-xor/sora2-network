import type { ApplyExtrinsicResult, Header, TransactionValidityError } from '@polkadot/types/interfaces';
import type { HexString } from '@polkadot/util/types';
import type { TaskCallResponse } from '../wasm-executor/index.js';
import { Block } from './block.js';
import type { InherentProvider } from './inherent/index.js';
import type { BuildBlockParams } from './txpool.js';
export declare const newHeader: (head: Block, unsafeBlockHeight?: number) => Promise<Header>;
export type BuildBlockCallbacks = {
    onApplyExtrinsicError?: (extrinsic: HexString, error: TransactionValidityError) => void;
    onPhaseApplied?: (phase: 'initialize' | 'finalize' | number, resp: TaskCallResponse) => void;
};
export declare const buildBlock: (head: Block, inherentProviders: InherentProvider[], params: BuildBlockParams, callbacks?: BuildBlockCallbacks) => Promise<[Block, HexString[]]>;
export type DryRunResult = {
    outcome: ApplyExtrinsicResult;
    storageDiff: [HexString, HexString | null][];
};
/**
 * Dry-run a batch of extrinsics against the same initialized block.
 *
 * Core_initialize_block and the inherents are executed once. Each extrinsic
 * is then applied on a temporary storage layer that is pushed before the call
 * and popped after, so no extrinsic side-effects leak into the next.
 *
 * Caveat: because account nonces revert with each pop, all extrinsics must be
 * pre-signed with the same base nonce.
 */
export declare const dryRunExtrinsicsAmortized: (head: Block, inherentProviders: InherentProvider[], extrinsics: HexString[], params: BuildBlockParams) => Promise<DryRunResult[]>;
export declare const dryRunExtrinsic: (head: Block, inherentProviders: InherentProvider[], extrinsic: HexString | {
    call: HexString;
    address: string;
}, params: BuildBlockParams) => Promise<TaskCallResponse>;
export declare const dryRunInherents: (head: Block, inherentProviders: InherentProvider[], params: BuildBlockParams) => Promise<[HexString, HexString | null][]>;
