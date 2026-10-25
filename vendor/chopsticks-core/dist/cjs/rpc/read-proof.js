"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "buildReadProof", {
    enumerable: true,
    get: function() {
        return buildReadProof;
    }
});
const _logger = require("../logger.js");
const _index = require("../utils/index.js");
const _index1 = require("../wasm-executor/index.js");
const _shared = require("./shared.js");
const logger = _logger.defaultLogger.child({
    name: 'rpc-read-proof'
});
const buildReadProof = async (context, keys, hash)=>{
    if (keys.length === 0) {
        throw new _shared.ResponseError(-32602, 'getReadProof requires a non-empty array of keys');
    }
    for (const key of keys){
        if ((0, _index.isPrefixedChildKey)(key)) {
            throw new _shared.ResponseError(-32601, `getReadProof does not support child-storage keys (got ${key}); use state_getChildReadProof`);
        }
    }
    const block = await context.chain.getBlock(hash);
    if (!block) {
        throw new _shared.ResponseError(1, `Block ${hash ?? 'head'} not found`);
    }
    // Upstream rejects chopsticks-only blocks with UnknownBlock; fall back to upstream
    // head whose trie is guaranteed available.
    const fetchUpstreamProof = async ()=>{
        try {
            return await context.chain.api.getReadProof(keys, block.hash);
        } catch (err) {
            logger.debug({
                err: err.message,
                blockHash: block.hash
            }, 'getReadProof at block failed; retrying at head');
            try {
                return await context.chain.api.getReadProof(keys);
            } catch (err2) {
                throw new _shared.ResponseError(-32603, `getReadProof: upstream rejected at block ${block.hash} and at head (${err2.message})`);
            }
        }
    };
    // The local reads and the upstream proof are independent — overlap their round trips.
    const [updates, upstreamProof] = await Promise.all([
        Promise.all(keys.map(async (key)=>[
                key,
                await block.get(key) ?? null
            ])),
        fetchUpstreamProof()
    ]);
    const { trieRootHash, nodes } = await (0, _index1.createProof)(upstreamProof.proof, updates);
    return {
        at: block.hash,
        proof: nodes,
        stateRoot: trieRootHash
    };
};
