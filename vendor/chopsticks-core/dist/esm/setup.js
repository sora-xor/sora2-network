import '@polkadot/types-codec';
import { HttpProvider, WsProvider } from '@polkadot/rpc-provider';
import { Api } from './api.js';
import { Blockchain } from './blockchain/index.js';
import { inherentProviders } from './blockchain/inherent/index.js';
import { defaultLogger } from './logger.js';
export const processOptions = async (options)=>{
    defaultLogger.debug(options, 'Setup options');
    let provider;
    if (options.genesis) {
        defaultLogger.debug(options, 'Setup options - genesis');
        provider = options.genesis;
    } else if (typeof options.endpoint === 'string' && /^(https|http):\/\//.test(options.endpoint || '')) {
        defaultLogger.debug(options, 'Setup options - HTTP');
        provider = new HttpProvider(options.endpoint);
    } else {
        defaultLogger.debug(options, 'Setup options - WS');
        provider = new WsProvider(options.endpoint, 3_000, undefined, options.rpcTimeout);
    }
    const api = new Api(provider);
    // setup api hooks
    api.onFetching(options.hooks?.apiFetching);
    if (options.db) {
        const scope = typeof options.endpoint === 'string' ? options.endpoint : JSON.stringify(options.endpoint ?? '');
        api.setDb(options.db, scope);
    }
    await api.isReady;
    let blockHash;
    if (options.block == null) {
        blockHash = await api.getFinalizedHead().then((hash)=>{
            if (!hash) {
                // should not happen, but just in case
                throw new Error('Cannot find block hash');
            }
            return hash;
        });
    } else if (typeof options.block === 'string' && options.block.startsWith('0x')) {
        blockHash = options.block;
    } else if (Number.isInteger(+options.block)) {
        blockHash = await api.getBlockHash(Number(options.block)).then((hash)=>{
            if (!hash) {
                throw new Error(`Cannot find block hash for ${options.block}`);
            }
            return hash;
        });
    } else {
        throw new Error(`Invalid block number or hash: ${options.block}`);
    }
    defaultLogger.debug({
        ...options,
        blockHash
    }, 'Args');
    return {
        ...options,
        blockHash,
        api
    };
};
export const setup = async (options)=>{
    const { api, blockHash, ...opts } = await processOptions(options);
    const header = await api.getHeader(blockHash);
    if (!header) {
        throw new Error(`Cannot find header for ${blockHash}`);
    }
    const chain = new Blockchain({
        api,
        buildBlockMode: opts.buildBlockMode,
        inherentProviders,
        db: opts.db,
        header: {
            hash: blockHash,
            number: Number(header.number)
        },
        mockSignatureHost: opts.mockSignatureHost,
        allowUnresolvedImports: opts.allowUnresolvedImports,
        runtimeLogLevel: opts.runtimeLogLevel,
        registeredTypes: opts.registeredTypes || {},
        offchainWorker: opts.offchainWorker,
        maxMemoryBlockCount: opts.maxMemoryBlockCount,
        processQueuedMessages: opts.processQueuedMessages,
        saveBlocks: opts.saveBlocks
    });
    if (opts.genesis) {
        // build 1st block
        await chain.newBlock();
    }
    return chain;
};
