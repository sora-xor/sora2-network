import { z } from 'zod';
import { defaultLogger } from '../logger.js';
export const logger = defaultLogger.child({
    name: 'rpc'
});
export const zHex = z.custom((val)=>/^0x\w+$/.test(val));
export const zHash = z.string().length(66).and(zHex);
export class ResponseError extends Error {
    code;
    constructor(code, message){
        super(message);
        this.name = 'ResponseError';
        this.code = code;
    }
    toJSON() {
        return {
            code: this.code,
            message: this.message
        };
    }
}
