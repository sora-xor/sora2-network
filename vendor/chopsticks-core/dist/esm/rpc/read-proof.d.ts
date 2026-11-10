import type { HexString } from '@polkadot/util/types';
import { type Context } from './shared.js';
/**
 * Compose a storage read proof: chopsticks keeps no trie of its own, so it fetches a base
 * proof from upstream (falling back to upstream head for chopsticks-only blocks) and
 * re-applies chopsticks-side values via `createProof`. Returns the proof nodes and the
 * recomputed trie root — which diverges from `chain_getHeader(at).state_root` once local
 * overrides are applied. Shared by `state_getReadProof` (drops the root, spec shape) and
 * `dev_getReadProof` (keeps it). Child-storage keys are rejected.
 */
export declare const buildReadProof: (context: Context, keys: HexString[], hash: HexString | undefined) => Promise<{
    at: HexString;
    proof: HexString[];
    stateRoot: HexString;
}>;
