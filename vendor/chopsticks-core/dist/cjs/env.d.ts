import * as z from 'zod';
export declare const environmentSchema: z.ZodObject<{
    DISABLE_AUTO_HRMP: z.ZodPipe<z.ZodDefault<z.ZodEnum<{
        false: "false";
        true: "true";
    }>>, z.ZodTransform<boolean, "false" | "true">>;
    PORT: z.ZodOptional<z.ZodString>;
    DISABLE_PLUGINS: z.ZodPipe<z.ZodDefault<z.ZodEnum<{
        false: "false";
        true: "true";
    }>>, z.ZodTransform<boolean, "false" | "true">>;
    HTTP_PROXY: z.ZodOptional<z.ZodString>;
    http_proxy: z.ZodOptional<z.ZodString>;
    HTTPS_PROXY: z.ZodOptional<z.ZodString>;
    https_proxy: z.ZodOptional<z.ZodString>;
    LOG_LEVEL: z.ZodDefault<z.ZodEnum<{
        debug: "debug";
        error: "error";
        fatal: "fatal";
        info: "info";
        trace: "trace";
        warn: "warn";
    }>>;
    VERBOSE_LOG: z.ZodPipe<z.ZodDefault<z.ZodEnum<{
        false: "false";
        true: "true";
    }>>, z.ZodTransform<boolean, "false" | "true">>;
    LOG_COMPACT: z.ZodPipe<z.ZodDefault<z.ZodEnum<{
        false: "false";
        true: "true";
    }>>, z.ZodTransform<boolean, "false" | "true">>;
}, z.core.$strip>;
/**
 * Environment variables available for users
 */
export declare const environment: {
    DISABLE_AUTO_HRMP: boolean;
    PORT?: string | undefined;
    DISABLE_PLUGINS: boolean;
    HTTP_PROXY?: string | undefined;
    http_proxy?: string | undefined;
    HTTPS_PROXY?: string | undefined;
    https_proxy?: string | undefined;
    LOG_LEVEL: "debug" | "error" | "fatal" | "info" | "trace" | "warn";
    VERBOSE_LOG: boolean;
    LOG_COMPACT: boolean;
};
