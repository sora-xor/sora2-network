// Structural interface for a caller-supplied light client. This package does not
// install or load any embedded light client; HTTP and WebSocket are self-contained.
export interface Config {
    forceEmbeddedNode?: boolean;
    embeddedNodeConfig?: { maxLogLevel?: number; workerFactory?: () => Worker };
}
export interface Chain {
    sendJsonRpc(rpc: string): void;
    remove(): void;
    addChain(spec: string, callback?: (response: string) => void, database?: string): Promise<Chain>;
}
export interface SubstrateConnect {
    WellKnownChain: Readonly<Record<string, string>>;
    createScClient(config?: Config): {
        addChain(spec: string, callback?: (response: string) => void, database?: string): Promise<Chain>;
        addWellKnownChain(id: string, callback?: (response: string) => void, database?: string): Promise<Chain>;
    };
}
