import { z } from 'zod';
export declare const genesisSchema: z.ZodObject<{
    id: z.ZodString;
    name: z.ZodString;
    properties: z.ZodObject<{
        ss58Format: z.ZodOptional<z.ZodNumber>;
        tokenDecimals: z.ZodOptional<z.ZodUnion<readonly [z.ZodNumber, z.ZodArray<z.ZodNumber>]>>;
        tokenSymbol: z.ZodOptional<z.ZodUnion<readonly [z.ZodString, z.ZodArray<z.ZodString>]>>;
    }, z.core.$strip>;
    genesis: z.ZodObject<{
        raw: z.ZodObject<{
            top: z.ZodRecord<z.ZodString, z.ZodString>;
        }, z.core.$strip>;
    }, z.core.$strip>;
}, z.core.$strip>;
export type Genesis = z.infer<typeof genesisSchema>;
