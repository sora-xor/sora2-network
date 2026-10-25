import { z } from 'zod';
import { type Context } from '../shared.js';
declare const schema: z.ZodObject<{
    count: z.ZodOptional<z.ZodNumber>;
    to: z.ZodOptional<z.ZodNumber>;
    dmp: z.ZodOptional<z.ZodArray<z.ZodObject<{
        sentAt: z.ZodNumber;
        msg: z.ZodCustom<`0x${string}`, `0x${string}`>;
    }, z.core.$strip>>>;
    ump: z.ZodOptional<z.ZodRecord<z.ZodNumber, z.ZodArray<z.ZodCustom<`0x${string}`, `0x${string}`>>>>;
    hrmp: z.ZodOptional<z.ZodRecord<z.ZodUnion<readonly [z.ZodNumber, z.ZodString]>, z.ZodArray<z.ZodObject<{
        sentAt: z.ZodNumber;
        data: z.ZodCustom<`0x${string}`, `0x${string}`>;
    }, z.core.$strip>>>>;
    transactions: z.ZodOptional<z.ZodArray<z.ZodCustom<`0x${string}`, `0x${string}`>>>;
    unsafeBlockHeight: z.ZodOptional<z.ZodNumber>;
    relayChainStateOverrides: z.ZodOptional<z.ZodArray<z.ZodTuple<[z.ZodCustom<`0x${string}`, `0x${string}`>, z.ZodUnion<readonly [z.ZodCustom<`0x${string}`, `0x${string}`>, z.ZodNull]>], null>>>;
    relayParentNumber: z.ZodOptional<z.ZodNumber>;
}, z.core.$strip>;
type Params = z.infer<typeof schema>;
export interface NewBlockParams {
    /**
     * The number of blocks to build
     */
    count: Params['count'];
    /**
     * The block number to build to
     */
    to: Params['to'];
    /**
     * The downward messages to include in the block
     */
    dmp: Params['dmp'];
    /**
     * The upward messages to include in the block
     */
    ump: Params['ump'];
    /**
     * The horizontal messages to include in the block
     */
    hrmp: Params['hrmp'];
    /**
     * The transactions to include in the block
     */
    transactions: Params['transactions'];
    /**
     * Build block using a specific block height (unsafe)
     */
    unsafeBlockHeight: Params['unsafeBlockHeight'];
    /**
     * Build block using a custom relay chain state
     */
    relayChainStateOverrides: Params['relayChainStateOverrides'];
    /**
     * Build block using a custom relay parent number
     */
    relayParentNumber: Params['relayParentNumber'];
}
/**
 * Build new blocks.
 *
 * This function is a dev rpc handler. Use `dev_newBlock` as the method name when calling it.
 *
 * @param context - The context object of the rpc handler
 * @param params - The parameters of the rpc handler
 *
 * @example Build 2 blocks
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * const ws = new WsProvider(`ws://localhost:8000`)
 * await ws.send('dev_newBlock', [{ count: 2 }])
 * ```
 * @example Build a block with upward messages
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * const ws = new WsProvider(`ws://localhost:8000`)
 * await ws.send('dev_newBlock', [
 *  {
 *    ump: {
 *      // https://acala.subscan.io/xcm_message/polkadot-ff66f28818d0b74573e62db8317e354b253fbc80
 *      2000: [
 *        '0x021000040000000007903fc4db080a130000000007903fc4db08000d010004000101009c4b11a0974cba4a395c94832fba812868a6cb0ba09e8519b3521093ea359905',
 *      ],
 *    }
 *  }
 * ])
 * ```
 *
 * @example Build two blocks with unsafeBlockHeight
 * ```ts
 * import { WsProvider } from '@polkadot/rpc-provider'
 * const ws = new WsProvider(`ws://localhost:8000`)
 * // this will create two blocks with block height 100000001 and 100000002
 * await ws.send('dev_newBlock', [{ count: 2, unsafeBlockHeight: 100000001 }])
 * ```
 */
export declare const dev_newBlock: (context: Context, [params]: [NewBlockParams]) => Promise<`0x${string}`>;
export {};
