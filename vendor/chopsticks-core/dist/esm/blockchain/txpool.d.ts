import type { HexString } from '@polkadot/util/types';
import { EventEmitter } from 'eventemitter3';
import type { Blockchain } from './index.js';
import type { InherentProvider } from './inherent/index.js';
export declare const APPLY_EXTRINSIC_ERROR = "TxPool::ApplyExtrinsicError";
export declare enum BuildBlockMode {
    /** One block per batch (default) */
    Batch = "Batch",
    /** One block per tx */
    Instant = "Instant",
    /** Only build when triggered */
    Manual = "Manual"
}
export interface DownwardMessage {
    sentAt: number;
    msg: HexString;
}
export interface HorizontalMessage {
    sentAt: number;
    data: HexString;
}
export interface BuildBlockParams {
    downwardMessages: DownwardMessage[];
    upwardMessages: Record<number, HexString[]>;
    horizontalMessages: Record<number, HorizontalMessage[]>;
    transactions: HexString[];
    unsafeBlockHeight?: number;
    relayChainStateOverrides?: [HexString, HexString | null][];
    relayParentNumber?: number;
}
export declare class TxPool {
    #private;
    readonly event: EventEmitter<string | symbol, any>;
    constructor(chain: Blockchain, inherentProviders: InherentProvider[], mode?: BuildBlockMode);
    get pendingExtrinsics(): HexString[];
    get ump(): Record<number, HexString[]>;
    get dmp(): DownwardMessage[];
    get hrmp(): Record<number, HorizontalMessage[]>;
    get mode(): BuildBlockMode;
    set mode(mode: BuildBlockMode);
    clear(): void;
    pendingExtrinsicsBy(address: string): HexString[];
    submitExtrinsic(extrinsic: HexString): Promise<void>;
    submitUpwardMessages(id: number, ump: HexString[]): void;
    submitDownwardMessages(dmp: DownwardMessage[]): void;
    submitHorizontalMessages(id: number, hrmp: HorizontalMessage[]): void;
    buildBlockWithParams(params: BuildBlockParams): Promise<void>;
    buildBlock(params?: Partial<BuildBlockParams>): Promise<void>;
    upcomingBlocks(): Promise<number>;
}
