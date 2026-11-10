import type { Block } from './block.js';
type Callback = (block: Block, pairs: [string, string | null][]) => void | Promise<void>;
export declare const randomId: () => string;
export declare class HeadState {
    #private;
    constructor(head: Block);
    subscribeHead(cb: (block: Block) => void): string;
    unsubscribeHead(id: string): void;
    subscribeStorage(keys: string[], cb: Callback): Promise<string>;
    unsubscribeStorage(id: string): void;
    setHead(head: Block): Promise<void>;
}
export {};
