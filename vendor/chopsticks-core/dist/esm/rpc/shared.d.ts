import { z } from 'zod';
import type { Blockchain } from '../blockchain/index.js';
export declare const logger: import("pino").Logger<never, boolean>;
export declare const zHex: z.ZodCustom<`0x${string}`, `0x${string}`>;
export declare const zHash: z.ZodIntersection<z.ZodString, z.ZodCustom<`0x${string}`, `0x${string}`>>;
export declare class ResponseError extends Error {
    code: number;
    constructor(code: number, message: string);
    toJSON(): {
        code: number;
        message: string;
    };
}
export interface Context {
    /**
     * The blockchain instance
     */
    chain: Blockchain;
}
export interface SubscriptionManager {
    subscribe: (method: string, subid: string, onCancel?: () => void) => (data: any) => void;
    unsubscribe: (subid: string) => void;
}
export type Handler<TParams = any, TReturn = any> = (context: Context, params: TParams, subscriptionManager: SubscriptionManager) => Promise<TReturn>;
export type Handlers = Record<string, Handler>;
