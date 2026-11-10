import type { HexString } from '@polkadot/util/types';
/** `ParaInfo { best_head_hash: { at_relay_block_number: u32, head_hash: H256 }, next_imported_hash_position: u32 }` */
export declare const encodeParaInfo: (atRelayBlockNumber: number, headHash: Uint8Array, nextImportedHashPosition: number) => HexString;
/** `ParaStoredHeaderData(Vec<u8>)` wrapping SCALE-encoded `StoredHeaderData { number: u32, state_root: H256 }`. */
export declare const encodeParaStoredHeaderData: (blockNumber: number, stateRoot: Uint8Array) => HexString;
