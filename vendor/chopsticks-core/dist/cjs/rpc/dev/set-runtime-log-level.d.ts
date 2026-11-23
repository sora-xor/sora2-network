import { type Context } from '../shared.js';
/**
 * Set runtime log level.
 *
 * This function is a dev rpc handler. Use `dev_setRuntimeLogLevel` as the method name when calling it.
 *
 * @param context - The context object of the rpc handler
 * @param runtimeLogLevel - The runtime log level to set
 *
 * @example Set runtime log level to 1
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * const ws = new WsProvider(`ws://localhost:8000`)
 * await ws.send('dev_setRuntimeLogLevel', [1])
 * ```
 */
export declare const dev_setRuntimeLogLevel: (context: Context, [runtimeLogLevel]: [number]) => Promise<void>;
