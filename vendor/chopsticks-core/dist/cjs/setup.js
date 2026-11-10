"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: Object.getOwnPropertyDescriptor(all, name).get
    });
}
_export(exports, {
    get processOptions () {
        return processOptions;
    },
    get setup () {
        return setup;
    }
});
require("@polkadot/types-codec");
const _rpcprovider = require("@polkadot/rpc-provider");
const _api = require("./api.js");
const _index = require("./blockchain/index.js");
const _index1 = require("./blockchain/inherent/index.js");
const _logger = require("./logger.js");
const processOptions = async (options)=>{
    _logger.defaultLogger.debug(options, 'Setup options');
    let provider;
    if (options.genesis) {
        _logger.defaultLogger.debug(options, 'Setup options - genesis');
        provider = options.genesis;
    } else if (typeof options.endpoint === 'string' && /^(https|http):\/\//.test(options.endpoint || '')) {
        _logger.defaultLogger.debug(options, 'Setup options - HTTP');
        provider = new _rpcprovider.HttpProvider(options.endpoint);
    } else {
        _logger.defaultLogger.debug(options, 'Setup options - WS');
        provider = new _rpcprovider.WsProvider(options.endpoint, 3_000, undefined, options.rpcTimeout);
    }
    const api = new _api.Api(provider);
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
    _logger.defaultLogger.debug({
        ...options,
        blockHash
    }, 'Args');
    return {
        ...options,
        blockHash,
        api
    };
};
const setup = async (options)=>{
    const { api, blockHash, ...opts } = await processOptions(options);
    const header = await api.getHeader(blockHash);
    if (!header) {
        throw new Error(`Cannot find header for ${blockHash}`);
    }
    const chain = new _index.Blockchain({
        api,
        buildBlockMode: opts.buildBlockMode,
        inherentProviders: _index1.inherentProviders,
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
