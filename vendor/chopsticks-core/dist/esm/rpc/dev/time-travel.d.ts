import { type Context } from '../shared.js';
/**
 * Travel to a specific time.
 *
 * This function is a dev rpc handler. Use `dev_timeTravel` as the method name when calling it.
 *
 * @param context - The context object of the rpc handler
 * @param date - Timestamp or date string to set
 *
 * @example
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * const ws = new WsProvider(`ws://localhost:8000`)
 * await ws.send('dev_timeTravel', ['Jan 1, 2023'])
 * ```
 */
export declare const dev_timeTravel: (context: Context, [date]: [string | number]) => Promise<number>;
