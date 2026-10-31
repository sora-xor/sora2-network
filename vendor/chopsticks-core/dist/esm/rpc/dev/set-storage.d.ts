import type { HexString } from '@polkadot/util/types';
import { type StorageValues } from '../../utils/set-storage.js';
import { type Context } from '../shared.js';
/**
 * Set storage values.
 *
 * This function is a dev rpc handler. Use `dev_setStorage` as the method name when calling it.
 *
 * @param context - The context object of the rpc handler
 * @param params - The parameters of the rpc handler
 *
 * @example
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * import { Keyring } from '@polkadot/keyring'
 *
 * const ws = new WsProvider(`ws://localhost:8000`)
 * const keyring = new Keyring({ type: 'ed25519' })
 * const bob = keyring.addFromUri('//Bob')
 *
 * const storage = {
 *   System: {
 *     Account: [[[bob.address], { data: { free: 100000 }, nonce: 1 }]],
 *   },
 * }
 * await ws.send('dev_setStorage', [storage])
 * ```
 */
export declare const dev_setStorage: (context: Context, params: [StorageValues, HexString?]) => Promise<`0x${string}`>;
