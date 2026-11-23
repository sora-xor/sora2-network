export declare const startWorker: <T>() => Promise<{
    remote: import("comlink").Remote<T>;
    terminate: () => Promise<void>;
}>;
