import { z } from 'zod';
import { type Context } from '../shared.js';
declare const schema: z.ZodUnion<[z.ZodIntersection<z.ZodString, z.ZodCustom<`0x${string}`, `0x${string}`>>, z.ZodNumber]>;
type Params = z.infer<typeof schema>;
/**
 * Set head.
 *
 * This function is a dev rpc handler. Use `dev_setHead` as the method name when calling it.
 *
 * @param context - The context object of the rpc handler
 * @param hashOrNumber - The block hash or number to set as head
 *
 * @example Set head to block 1000000
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * const ws = new WsProvider(`ws://localhost:8000`)
 * await ws.send('dev_setHead', [1000000])
 * ```
 */
export declare const dev_setHead: (context: Context, [params]: [Params]) => Promise<`0x${string}`>;
export {};
