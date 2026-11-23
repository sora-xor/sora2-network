"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "Blockchain", {
    enumerable: true,
    get: function() {
        return Blockchain;
    }
});
const _types = require("@polkadot/types");
const _util = require("@polkadot/types-known/util");
const _util1 = require("@polkadot/util");
const _utilcrypto = require("@polkadot/util-crypto");
const _lodash = /*#__PURE__*/ _interop_require_default(require("lodash"));
const _logger = require("../logger.js");
const _offchain = require("../offchain.js");
const _index = require("../utils/index.js");
const _block = require("./block.js");
const _blockbuilder = require("./block-builder.js");
const _headstate = require("./head-state.js");
const _txpool = require("./txpool.js");
function _check_private_redeclaration(obj, privateCollection) {
    if (privateCollection.has(obj)) {
        throw new TypeError("Cannot initialize the same private elements twice on an object");
    }
}
function _class_apply_descriptor_get(receiver, descriptor) {
    if (descriptor.get) {
        return descriptor.get.call(receiver);
    }
    return descriptor.value;
}
function _class_apply_descriptor_set(receiver, descriptor, value) {
    if (descriptor.set) {
        descriptor.set.call(receiver, value);
    } else {
        if (!descriptor.writable) {
            throw new TypeError("attempted to set read only private field");
        }
        descriptor.value = value;
    }
}
function _class_extract_field_descriptor(receiver, privateMap, action) {
    if (!privateMap.has(receiver)) {
        throw new TypeError("attempted to " + action + " private field on non-instance");
    }
    return privateMap.get(receiver);
}
function _class_private_field_get(receiver, privateMap) {
    var descriptor = _class_extract_field_descriptor(receiver, privateMap, "get");
    return _class_apply_descriptor_get(receiver, descriptor);
}
function _class_private_field_init(obj, privateMap, value) {
    _check_private_redeclaration(obj, privateMap);
    privateMap.set(obj, value);
}
function _class_private_field_set(receiver, privateMap, value) {
    var descriptor = _class_extract_field_descriptor(receiver, privateMap, "set");
    _class_apply_descriptor_set(receiver, descriptor, value);
    return value;
}
function _class_private_method_get(receiver, privateSet, fn) {
    if (!privateSet.has(receiver)) {
        throw new TypeError("attempted to get private field on non-instance");
    }
    return fn;
}
function _class_private_method_init(obj, privateSet) {
    _check_private_redeclaration(obj, privateSet);
    privateSet.add(obj);
}
function _define_property(obj, key, value) {
    if (key in obj) {
        Object.defineProperty(obj, key, {
            value: value,
            enumerable: true,
            configurable: true,
            writable: true
        });
    } else {
        obj[key] = value;
    }
    return obj;
}
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const logger = _logger.defaultLogger.child({
    name: 'blockchain'
});
var _runtimeLogLevel = /*#__PURE__*/ new WeakMap(), _txpool1 = /*#__PURE__*/ new WeakMap(), _inherentProviders = /*#__PURE__*/ new WeakMap(), _head = /*#__PURE__*/ new WeakMap(), _blocksByNumber = /*#__PURE__*/ new WeakMap(), _blocksByHash = /*#__PURE__*/ new WeakMap(), _loadingBlocks = /*#__PURE__*/ new WeakMap(), _maxMemoryBlockCount = /*#__PURE__*/ new WeakMap(), _registryBuilder = /*#__PURE__*/ new WeakMap(), _registerBlock = /*#__PURE__*/ new WeakSet();
class Blockchain {
    get head() {
        return _class_private_field_get(this, _head);
    }
    get txPool() {
        return _class_private_field_get(this, _txpool1);
    }
    get runtimeLogLevel() {
        return _class_private_field_get(this, _runtimeLogLevel);
    }
    set runtimeLogLevel(level) {
        _class_private_field_set(this, _runtimeLogLevel, level);
        logger.debug(`Runtime log level set to ${logger.level}`);
    }
    async buildRegistry(metadata, version) {
        const cacheKey = `${(0, _utilcrypto.xxhashAsHex)(metadata, 256)}-${version.specVersion}`;
        return _class_private_field_get(this, _registryBuilder).call(this, cacheKey, metadata, version);
    }
    async saveBlockToDB(block) {
        if (this.db && this.saveBlocks) {
            const { hash, number, header, extrinsics } = block;
            // delete old ones with the same block number if any, keep the latest one
            await this.db.saveBlock({
                hash,
                number,
                header: (await header).toHex(),
                extrinsics: await extrinsics,
                parentHash: (await block.parentBlock)?.hash || null,
                storageDiff: await block.storageDiff()
            });
        }
    }
    /**
   * Try to load block from db and register it.
   * If pass in number, get block by number, else get block by hash.
   */ async loadBlockFromDB(hashOrNumber) {
        if (this.db) {
            const blockData = typeof hashOrNumber === 'number' ? await this.db.queryBlockByNumber(hashOrNumber) : await this.db.queryBlock(hashOrNumber);
            if (blockData) {
                const { hash, number, header, extrinsics } = blockData;
                const parentHash = blockData.parentHash || undefined;
                let parentBlock = parentHash ? _class_private_field_get(this, _blocksByHash).get(parentHash) : undefined;
                if (!parentBlock) {
                    parentBlock = await this.getBlock(parentHash);
                }
                const storageDiff = blockData.storageDiff ?? undefined;
                const registry = await this.head.registry;
                const block = new _block.Block(this, number, hash, parentBlock, {
                    header: registry.createType('Header', header),
                    extrinsics,
                    storage: parentBlock?.storage,
                    storageDiff
                });
                _class_private_method_get(this, _registerBlock, registerBlock).call(this, block);
                return block;
            }
        }
        return undefined;
    }
    /**
   * Get block by number.
   */ async getBlockAt(number) {
        if (number === null || number === undefined) {
            return this.head;
        }
        if (number > _class_private_field_get(this, _head).number) {
            return undefined;
        }
        if (!_class_private_field_get(this, _blocksByNumber).has(number)) {
            const blockFromDB = await this.loadBlockFromDB(number);
            if (blockFromDB) {
                return blockFromDB;
            }
            const hash = await this.api.getBlockHash(number);
            if (!hash) {
                return undefined;
            }
            const block = new _block.Block(this, number, hash);
            _class_private_method_get(this, _registerBlock, registerBlock).call(this, block);
        }
        return _class_private_field_get(this, _blocksByNumber).get(number);
    }
    /**
   * Get block by hash.
   */ async getBlock(hash) {
        await this.api.isReady;
        if (hash == null) {
            hash = this.head.hash;
        }
        if (!_class_private_field_get(this, _blocksByHash).has(hash)) {
            const loadingBlock = _class_private_field_get(this, _loadingBlocks)[hash];
            if (loadingBlock) {
                await loadingBlock;
            } else {
                const loadingBlock = (async ()=>{
                    try {
                        const blockFromDB = await this.loadBlockFromDB(hash);
                        if (!blockFromDB) {
                            const header = await this.api.getHeader(hash);
                            if (!header) {
                                throw new Error(`Block ${hash} not found`);
                            }
                            const block = new _block.Block(this, Number(header.number), hash);
                            _class_private_method_get(this, _registerBlock, registerBlock).call(this, block);
                        }
                    } catch (e) {
                        logger.debug(`getBlock(${hash}) failed: ${e}`);
                    }
                })();
                _class_private_field_get(this, _loadingBlocks)[hash] = loadingBlock;
                await loadingBlock;
                delete _class_private_field_get(this, _loadingBlocks)[hash];
            }
        }
        return _class_private_field_get(this, _blocksByHash).get(hash);
    }
    /**
   * Get all blocks in memory.
   */ blocksInMemory() {
        return Array.from(_class_private_field_get(this, _blocksByNumber).values());
    }
    /**
   * Remove block from memory and db.
   */ async unregisterBlock(block) {
        if (block.hash === this.head.hash) {
            throw new Error('Cannot unregister head block');
        }
        if (_class_private_field_get(this, _blocksByNumber).get(block.number)?.hash === block.hash) {
            _class_private_field_get(this, _blocksByNumber).delete(block.number);
        }
        _class_private_field_get(this, _blocksByHash).delete(block.hash);
        // delete from db
        if (this.db) {
            await this.db.deleteBlock(block.hash);
        }
    }
    async onNewBlock(block) {
        await this.setHead(block);
        await this.saveBlockToDB(block);
    }
    /**
   * Set block as head.
   */ async setHead(block) {
        logger.debug({
            number: block.number,
            hash: block.hash
        }, 'setHead');
        _class_private_field_set(this, _head, block);
        _class_private_method_get(this, _registerBlock, registerBlock).call(this, block);
        await this.headState.setHead(block);
        if (this.offchainWorker) {
            await this.offchainWorker.run(block);
        }
    }
    /**
   * Submit extrinsic to txpool.
   */ async submitExtrinsic(extrinsic) {
        const validity = await this.validateExtrinsic(extrinsic);
        if (validity.isOk) {
            await _class_private_field_get(this, _txpool1).submitExtrinsic(extrinsic);
            return (0, _utilcrypto.blake2AsHex)(extrinsic, 256);
        }
        throw validity.asErr;
    }
    /**
   * Validate extrinsic by calling `TaggedTransactionQueue_validate_transaction`.
   */ async validateExtrinsic(extrinsic, source = '0x02' /** External */ ) {
        const args = (0, _util1.u8aToHex)((0, _util1.u8aConcat)(source, extrinsic, this.head.hash));
        const res = await this.head.call('TaggedTransactionQueue_validate_transaction', [
            args
        ]);
        const registry = await this.head.registry;
        return registry.createType('TransactionValidity', res.result);
    }
    submitUpwardMessages(id, ump) {
        _class_private_field_get(this, _txpool1).submitUpwardMessages(id, ump);
        logger.debug({
            id,
            ump
        }, 'submitUpwardMessages');
    }
    submitDownwardMessages(dmp) {
        _class_private_field_get(this, _txpool1).submitDownwardMessages(dmp);
        logger.debug({
            dmp
        }, 'submitDownwardMessages');
    }
    submitHorizontalMessages(id, hrmp) {
        _class_private_field_get(this, _txpool1).submitHorizontalMessages(id, hrmp);
        logger.debug({
            id,
            hrmp
        }, 'submitHorizontalMessages');
    }
    /**
   * Build a new block with optional params. Use this when you don't have all the {@link BuildBlockParams}
   */ async newBlock(params) {
        await _class_private_field_get(this, _txpool1).buildBlock(params);
        return _class_private_field_get(this, _head);
    }
    /**
   * Build a new block with {@link BuildBlockParams}.
   */ async newBlockWithParams(params) {
        await _class_private_field_get(this, _txpool1).buildBlockWithParams(params);
        return _class_private_field_get(this, _head);
    }
    /**
   * Get the upcoming blocks.
   */ async upcomingBlocks() {
        return _class_private_field_get(this, _txpool1).upcomingBlocks();
    }
    /**
   * Dry run extrinsic in block `at`.
   */ async dryRunExtrinsic(extrinsic, at) {
        await this.api.isReady;
        const head = at ? await this.getBlock(at) : this.head;
        if (!head) {
            throw new Error(`Cannot find block ${at}`);
        }
        const registry = await head.registry;
        const params = {
            transactions: [],
            downwardMessages: [],
            upwardMessages: [],
            horizontalMessages: {}
        };
        const { result, storageDiff } = await (0, _blockbuilder.dryRunExtrinsic)(head, _class_private_field_get(this, _inherentProviders), extrinsic, params);
        const outcome = registry.createType('ApplyExtrinsicResult', result);
        return {
            outcome,
            storageDiff
        };
    }
    /**
   * Dry-run a batch of extrinsics in block `at`, amortizing block initialization
   * across the whole batch. Each extrinsic is applied independently — its storage
   * changes do not accumulate into subsequent extrinsics.
   */ async dryRunExtrinsicsAmortized(extrinsics, at) {
        await this.api.isReady;
        const head = at ? await this.getBlock(at) : this.head;
        if (!head) {
            throw new Error(`Cannot find block ${at}`);
        }
        const params = {
            transactions: [],
            downwardMessages: [],
            upwardMessages: [],
            horizontalMessages: {}
        };
        return (0, _blockbuilder.dryRunExtrinsicsAmortized)(head, _class_private_field_get(this, _inherentProviders), extrinsics, params);
    }
    /**
   * Dry run hrmp messages in block `at`.
   * Return the storage diff.
   */ async dryRunHrmp(hrmp, at) {
        await this.api.isReady;
        const head = at ? await this.getBlock(at) : this.head;
        if (!head) {
            throw new Error(`Cannot find block ${at}`);
        }
        const params = {
            transactions: [],
            downwardMessages: [],
            upwardMessages: [],
            horizontalMessages: hrmp
        };
        return (0, _blockbuilder.dryRunInherents)(head, _class_private_field_get(this, _inherentProviders), params);
    }
    /**
   * Dry run dmp messages in block `at`.
   * Return the storage diff.
   */ async dryRunDmp(dmp, at) {
        await this.api.isReady;
        const head = at ? await this.getBlock(at) : this.head;
        if (!head) {
            throw new Error(`Cannot find block ${at}`);
        }
        const params = {
            transactions: [],
            downwardMessages: dmp,
            upwardMessages: [],
            horizontalMessages: {}
        };
        return (0, _blockbuilder.dryRunInherents)(head, _class_private_field_get(this, _inherentProviders), params);
    }
    /**
   * Dry run ump messages in block `at`.
   * Return the storage diff.
   */ async dryRunUmp(ump, at) {
        await this.api.isReady;
        const head = at ? await this.getBlock(at) : this.head;
        if (!head) {
            throw new Error(`Cannot find block ${at}`);
        }
        const meta = await head.meta;
        const needsDispatch = meta.registry.createType('Vec<u32>', Object.keys(ump));
        const storageValues = [
            [
                (0, _index.compactHex)(meta.query.ump.needsDispatch()),
                needsDispatch.toHex()
            ]
        ];
        for (const [paraId, messages] of Object.entries(ump)){
            const upwardMessages = meta.registry.createType('Vec<Bytes>', messages);
            if (upwardMessages.length === 0) throw new Error('No upward meesage');
            const queueSize = meta.registry.createType('(u32, u32)', [
                upwardMessages.length,
                upwardMessages.map((x)=>x.byteLength).reduce((s, i)=>s + i, 0)
            ]);
            storageValues.push([
                (0, _index.compactHex)(meta.query.ump.relayDispatchQueues(paraId)),
                upwardMessages.toHex()
            ]);
            storageValues.push([
                (0, _index.compactHex)(meta.query.ump.relayDispatchQueueSize(paraId)),
                queueSize.toHex()
            ]);
        }
        head.pushStorageLayer().setAll(storageValues);
        const params = {
            transactions: [],
            downwardMessages: [],
            upwardMessages: [],
            horizontalMessages: {}
        };
        return (0, _blockbuilder.dryRunInherents)(head, _class_private_field_get(this, _inherentProviders), params);
    }
    /**
   * Get inherents of head.
   */ getInherents() {
        return _class_private_field_get(this, _inherentProviders);
    }
    /**
   * Close the db and disconnect api.
   */ async close() {
        await this.api.disconnect();
        await this.db?.close();
    }
    /**
   * @param options - Options for instantiating the blockchain
   */ constructor({ api, buildBlockMode, inherentProviders, db, header, mockSignatureHost = false, allowUnresolvedImports = false, runtimeLogLevel = 0, registeredTypes = {}, offchainWorker = false, maxMemoryBlockCount = 500, processQueuedMessages = true, saveBlocks = true }){
        _class_private_method_init(this, _registerBlock);
        /** API instance, for getting on-chain data. */ _define_property(this, "api", void 0);
        /** Datasource for caching storage and blocks data. */ _define_property(this, "db", void 0);
        /** Enable mock signature. Any signature starts with 0xdeadbeef and filled by 0xcd is considered valid */ _define_property(this, "mockSignatureHost", void 0);
        /** Allow wasm unresolved imports. */ _define_property(this, "allowUnresolvedImports", void 0);
        _class_private_field_init(this, _runtimeLogLevel, {
            writable: true,
            value: void 0
        });
        /** Polkadot.js custom types registration. */ _define_property(this, "registeredTypes", void 0);
        _class_private_field_init(this, _txpool1, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _inherentProviders, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _head, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _blocksByNumber, {
            writable: true,
            value: new Map()
        });
        _class_private_field_init(this, _blocksByHash, {
            writable: true,
            value: new Map()
        });
        _class_private_field_init(this, _loadingBlocks, {
            writable: true,
            value: {}
        });
        /** For subscribing and managing the head state. */ _define_property(this, "headState", void 0);
        _define_property(this, "offchainWorker", void 0);
        _class_private_field_init(this, _maxMemoryBlockCount, {
            writable: true,
            value: void 0
        });
        _define_property(this, "processQueuedMessages", true);
        _define_property(this, "saveBlocks", void 0);
        // first arg is used as cache key
        _class_private_field_init(this, _registryBuilder, {
            writable: true,
            value: _lodash.default.memoize(async (_cacheKey, metadata, version)=>{
                const chain = await this.api.chain;
                const properties = await this.api.chainProperties;
                const registry = new _types.TypeRegistry();
                registry.setKnownTypes(this.registeredTypes);
                registry.setChainProperties(registry.createType('ChainProperties', properties));
                registry.register((0, _util.getSpecTypes)(registry, chain, version.specName, version.specVersion));
                registry.setHasher((0, _util.getSpecHasher)(registry, chain, version.specName));
                registry.setMetadata(new _types.Metadata(registry, metadata), undefined, (0, _util1.objectSpread)({}, (0, _util.getSpecExtensions)(registry, chain, version.specName), this.api.signedExtensions), true);
                return registry;
            })
        });
        this.api = api;
        this.db = db;
        this.mockSignatureHost = mockSignatureHost;
        this.allowUnresolvedImports = allowUnresolvedImports;
        _class_private_field_set(this, _runtimeLogLevel, runtimeLogLevel);
        this.registeredTypes = registeredTypes;
        _class_private_field_set(this, _head, new _block.Block(this, header.number, header.hash));
        _class_private_method_get(this, _registerBlock, registerBlock).call(this, _class_private_field_get(this, _head));
        _class_private_field_set(this, _txpool1, new _txpool.TxPool(this, inherentProviders, buildBlockMode));
        _class_private_field_set(this, _inherentProviders, inherentProviders);
        this.headState = new _headstate.HeadState(_class_private_field_get(this, _head));
        if (offchainWorker) {
            this.offchainWorker = new _offchain.OffchainWorker();
        }
        _class_private_field_set(this, _maxMemoryBlockCount, maxMemoryBlockCount);
        this.processQueuedMessages = processQueuedMessages;
        this.saveBlocks = saveBlocks;
    }
}
function registerBlock(block) {
    // if exceed max memory block count, delete the oldest block
    if (_class_private_field_get(this, _blocksByNumber).size === _class_private_field_get(this, _maxMemoryBlockCount)) {
        // #blocksByNumber can't be empty so the force unwrap is safe
        const { hash, number } = _class_private_field_get(this, _blocksByNumber).values().next().value;
        _class_private_field_get(this, _blocksByNumber).delete(number);
        _class_private_field_get(this, _blocksByHash).delete(hash);
    }
    _class_private_field_get(this, _blocksByNumber).set(block.number, block);
    _class_private_field_get(this, _blocksByHash).set(block.hash, block);
}
