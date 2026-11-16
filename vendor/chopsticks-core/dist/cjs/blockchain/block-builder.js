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
    get buildBlock () {
        return buildBlock;
    },
    get dryRunExtrinsic () {
        return dryRunExtrinsic;
    },
    get dryRunExtrinsicsAmortized () {
        return dryRunExtrinsicsAmortized;
    },
    get dryRunInherents () {
        return dryRunInherents;
    },
    get newHeader () {
        return newHeader;
    }
});
const _util = require("@polkadot/util");
const _utilcrypto = require("@polkadot/util-crypto");
const _logger = require("../logger.js");
const _index = require("../utils/index.js");
const _block = require("./block.js");
const _storagelayer = require("./storage-layer.js");
const logger = _logger.defaultLogger.child({
    name: 'block-builder'
});
const genesisDigestLogs = async (head)=>{
    const meta = await head.meta;
    const currentSlot = await (0, _index.getCurrentSlot)(head);
    if (meta.consts.babe) {
        const newSlot = meta.registry.createType('Slot', currentSlot + 1);
        const consensusEngine = meta.registry.createType('ConsensusEngineId', 'BABE');
        const preDigest = meta.registry.createType('RawBabePreDigest', {
            SecondaryVRF: {
                authorityIndex: 514,
                slotNumber: newSlot,
                vrfOutput: '0x44cadd14aaefbda13ac8d85e1a6d58be082e7e2f56a4f95a3c612c784aaa4063',
                vrfProof: '0xf5517bf67d93ce633cde2fde7fbcf8ddca80017aaf8cd48436514687c662f60eda0ffa2c4781906416f4e71a196c9783c60c1b83d54c3a29365d03706714570b'
            }
        });
        const digest = meta.registry.createType('DigestItem', {
            PreRuntime: [
                consensusEngine,
                (0, _util.compactAddLength)(preDigest.toU8a())
            ]
        });
        return [
            digest
        ];
    }
    const newSlot = meta.registry.createType('Slot', currentSlot + 1);
    const consensusEngine = meta.registry.createType('ConsensusEngineId', 'aura');
    const digest = meta.registry.createType('DigestItem', {
        PreRuntime: [
            consensusEngine,
            (0, _util.compactAddLength)(newSlot.toU8a())
        ]
    });
    return [
        digest
    ];
};
const babePreDigestSetSlot = (digest, slotNumber)=>{
    if (digest.isPrimary) {
        return {
            primary: {
                ...digest.asPrimary.toJSON(),
                slotNumber
            }
        };
    }
    if (digest.isSecondaryPlain) {
        return {
            secondaryPlain: {
                ...digest.asSecondaryPlain.toJSON(),
                slotNumber
            }
        };
    }
    if (digest.isSecondaryVRF) {
        return {
            secondaryVRF: {
                ...digest.asSecondaryVRF.toJSON(),
                slotNumber
            }
        };
    }
    return digest.toJSON();
};
const newHeader = async (head, unsafeBlockHeight)=>{
    const meta = await head.meta;
    const parentHeader = await head.header;
    let newLogs = !head.number ? await genesisDigestLogs(head) : parentHeader.digest.logs.toArray();
    newLogs = await Promise.all(newLogs.map(async (item)=>{
        if (item.isPreRuntime) {
            const [consensusEngine, preDigest] = item.asPreRuntime;
            if (consensusEngine.isAura) {
                const slot = await (0, _index.getCurrentSlot)(head);
                const newSlot = (0, _util.compactAddLength)(meta.registry.createType('Slot', slot + 1).toU8a());
                return meta.registry.createType('DigestItem', {
                    PreRuntime: [
                        consensusEngine,
                        newSlot
                    ]
                });
            } else if (consensusEngine.isBabe) {
                const slot = await (0, _index.getCurrentSlot)(head);
                const digest = meta.registry.createType('RawBabePreDigest', preDigest);
                const newSlot = (0, _util.compactAddLength)(meta.registry.createType('RawBabePreDigest', babePreDigestSetSlot(digest, slot + 1)).toU8a());
                return meta.registry.createType('DigestItem', {
                    PreRuntime: [
                        consensusEngine,
                        newSlot
                    ]
                });
            } else if (consensusEngine?.toString() === 'nmbs') {
                const nmbsKey = (0, _util.stringToHex)('nmbs');
                if (meta.query.randomness?.notFirstBlock) {
                    // TODO: shouldn't modify existing head
                    // reset notFirstBlock so randomness will skip validation
                    head.pushStorageLayer().set((0, _index.compactHex)(meta.query.randomness.notFirstBlock()), _storagelayer.StorageValueKind.Deleted);
                }
                return meta.registry.createType('DigestItem', {
                    // Using previous block author
                    PreRuntime: [
                        consensusEngine,
                        parentHeader.digest.logs.find((log)=>log.isPreRuntime && log.asPreRuntime[0].toHex() === nmbsKey)?.asPreRuntime[1].toHex()
                    ]
                });
            }
        }
        return item;
    }));
    const header = meta.registry.createType('Header', {
        parentHash: head.hash,
        number: unsafeBlockHeight ?? head.number + 1,
        stateRoot: '0x0000000000000000000000000000000000000000000000000000000000000000',
        extrinsicsRoot: '0x0000000000000000000000000000000000000000000000000000000000000000',
        digest: {
            logs: newLogs
        }
    });
    return header;
};
const initNewBlock = async (head, header, inherentProviders, params, storageLayer, callback)=>{
    const blockNumber = header.number.toNumber();
    const { unsafeBlockHeight } = params;
    const hash = `0x${Math.round(Math.random() * 100000000).toString(16).padEnd(64, '0')}`;
    const newBlock = new _block.Block(head.chain, blockNumber, hash, head, {
        header,
        extrinsics: [],
        storage: storageLayer ?? head.storage
    });
    {
        // override block number in storage when using unsafeBlockHeight
        // this is needed because the runtime validates that block numbers are strictly increasing
        if (unsafeBlockHeight !== undefined && blockNumber > head.number + 1) {
            const meta = await head.meta;
            const value = meta.registry.createType('BlockNumber', blockNumber - 1).toU8a();
            newBlock.pushStorageLayer().set((0, _index.compactHex)(meta.query.system.number()), (0, _util.u8aToHex)(value));
        }
        // initialize block
        const resp = await newBlock.call('Core_initialize_block', [
            header.toHex()
        ]);
        newBlock.pushStorageLayer().setAll(resp.storageDiff);
        if (head.number === 0) {
            // set parent hash for genesis block
            // this makes sure to override the default parent hash
            const meta = await head.meta;
            const header = await head.header;
            newBlock.pushStorageLayer().setAll([
                [
                    (0, _index.compactHex)(meta.query.system.parentHash()),
                    header.hash.toHex()
                ]
            ]);
        }
        callback?.onPhaseApplied?.('initialize', resp);
    }
    const inherents = [];
    const layers = [];
    // apply inherents
    for (const inherentProvider of inherentProviders){
        try {
            const extrinsics = await inherentProvider.createInherents(newBlock, params);
            if (extrinsics.length === 0) {
                continue;
            }
            // bypass signature check during inherent extrinsics
            // this is needed to allow cumulus to accept fake relay block digests
            // this should be safe because there are no valid use of invalid signatures in inherents
            const resp = await newBlock.call('BlockBuilder_apply_extrinsic', extrinsics, true);
            const layer = newBlock.pushStorageLayer();
            layer.setAll(resp.storageDiff);
            layers.push(layer);
            inherents.push(...extrinsics);
            callback?.onPhaseApplied?.(layers.length - 1, resp);
        } catch (e) {
            logger.warn({
                err: e
            }, 'Failed to apply inherents');
            throw new Error('Failed to apply inherents');
        }
    }
    return {
        block: newBlock,
        layers,
        inherents
    };
};
const buildBlock = async (head, inherentProviders, params, callbacks)=>{
    const { transactions: extrinsics, upwardMessages: ump, unsafeBlockHeight } = params;
    const registry = await head.registry;
    const header = await newHeader(head, unsafeBlockHeight);
    const newBlockNumber = header.number.toNumber();
    logger.info({
        number: newBlockNumber,
        extrinsics: extrinsics.map(_logger.truncate),
        umpCount: Object.keys(ump).length
    }, `${await head.chain.api.getSystemChain()} building #${newBlockNumber.toLocaleString()}`);
    let layer;
    // apply ump via storage override hack
    if (Object.keys(ump).length > 0) {
        const meta = await head.meta;
        layer = new _storagelayer.StorageLayer(head.storage);
        for (const [paraId, upwardMessages] of Object.entries(ump)){
            const upwardMessagesU8a = upwardMessages.map((x)=>(0, _util.hexToU8a)(x));
            const messagesCount = upwardMessages.length;
            const messagesSize = upwardMessagesU8a.map((x)=>x.length).reduce((s, i)=>s + i, 0);
            if (meta.query.ump) {
                const queueSize = meta.registry.createType('(u32, u32)', [
                    messagesCount,
                    messagesSize
                ]);
                const messages = meta.registry.createType('Vec<Bytes>', upwardMessages);
                // TODO: make sure we append instead of replace
                layer.setAll([
                    [
                        (0, _index.compactHex)(meta.query.ump.relayDispatchQueues(paraId)),
                        messages.toHex()
                    ],
                    [
                        (0, _index.compactHex)(meta.query.ump.relayDispatchQueueSize(paraId)),
                        queueSize.toHex()
                    ]
                ]);
            } else if (meta.query.messageQueue) {
                // TODO: make sure we append instead of replace
                const origin = {
                    ump: {
                        para: paraId
                    }
                };
                let last = 0;
                let heap = new Uint8Array(0);
                for (const message of upwardMessagesU8a){
                    const payloadLen = message.length;
                    const header = meta.registry.createType('(u32, bool)', [
                        payloadLen,
                        false
                    ]);
                    last = heap.length;
                    heap = (0, _util.u8aConcat)(heap, header.toU8a(), message);
                }
                layer.setAll([
                    [
                        (0, _index.compactHex)(meta.query.messageQueue.bookStateFor(origin)),
                        meta.registry.createType('PalletMessageQueueBookState', {
                            begin: 0,
                            end: 1,
                            count: 1,
                            readyNeighbours: {
                                prev: origin,
                                next: origin
                            },
                            messageCount: messagesCount,
                            size_: messagesSize
                        }).toHex()
                    ],
                    [
                        (0, _index.compactHex)(meta.query.messageQueue.serviceHead(origin)),
                        meta.registry.createType('PolkadotRuntimeParachainsInclusionAggregateMessageOrigin', origin).toHex()
                    ],
                    [
                        (0, _index.compactHex)(meta.query.messageQueue.pages(origin, 0)),
                        meta.registry.createType('PalletMessageQueuePage', {
                            remaining: messagesCount,
                            remaining_size: messagesSize,
                            first_index: 0,
                            first: 0,
                            last,
                            heap: (0, _util.compactAddLength)(heap)
                        }).toHex()
                    ]
                ]);
            } else {
                throw new Error('Unknown ump storage');
            }
            logger.trace({
                paraId,
                upwardMessages: (0, _logger.truncate)(upwardMessages)
            }, 'Pushed UMP');
        }
        if (meta.query.ump) {
            const needsDispatch = meta.registry.createType('Vec<u32>', Object.keys(ump));
            layer.set((0, _index.compactHex)(meta.query.ump.needsDispatch()), needsDispatch.toHex());
        }
    }
    const { block: newBlock, inherents } = await initNewBlock(head, header, inherentProviders, params, layer);
    const pendingExtrinsics = [];
    const includedExtrinsic = [];
    // apply extrinsics
    for (const extrinsic of extrinsics){
        try {
            const resp = await newBlock.call('BlockBuilder_apply_extrinsic', [
                extrinsic
            ]);
            const outcome = registry.createType('ApplyExtrinsicResult', resp.result);
            if (outcome.isErr) {
                callbacks?.onApplyExtrinsicError?.(extrinsic, outcome.asErr);
                continue;
            }
            newBlock.pushStorageLayer().setAll(resp.storageDiff);
            includedExtrinsic.push(extrinsic);
            callbacks?.onPhaseApplied?.(includedExtrinsic.length - 1, resp);
        } catch (e) {
            logger.info({
                err: e
            }, 'Failed to apply extrinsic');
            pendingExtrinsics.push(extrinsic);
        }
    }
    {
        // finalize block
        const resp = await newBlock.call('BlockBuilder_finalize_block', []);
        newBlock.pushStorageLayer().setAll(resp.storageDiff);
        callbacks?.onPhaseApplied?.('finalize', resp);
    }
    const allExtrinsics = [
        ...inherents,
        ...includedExtrinsic
    ];
    const mockExtrinsicRoot = (0, _utilcrypto.blake2AsU8a)((0, _util.u8aConcat)(...allExtrinsics), 256);
    const finalHeader = registry.createType('Header', {
        ...header.toJSON(),
        extrinsicsRoot: mockExtrinsicRoot
    });
    const storageDiff = await newBlock.storageDiff();
    if (logger.level.toLowerCase() === 'trace') {
        logger.trace(Object.entries(storageDiff).map(([key, value])=>[
                key,
                (0, _logger.truncate)(value)
            ]), 'Final block');
    }
    const finalBlock = new _block.Block(head.chain, newBlock.number, finalHeader.hash.toHex(), head, {
        header: finalHeader,
        extrinsics: allExtrinsics,
        storage: head.storage,
        storageDiff
    });
    logger.info({
        number: finalBlock.number,
        hash: finalBlock.hash,
        extrinsics: (0, _logger.truncate)(includedExtrinsic),
        pendingExtrinsics: pendingExtrinsics.map(_logger.truncate),
        ump: (0, _logger.truncate)(ump)
    }, `${await head.chain.api.getSystemChain()} new head #${finalBlock.number.toLocaleString()}`);
    return [
        finalBlock,
        pendingExtrinsics
    ];
};
const dryRunExtrinsicsAmortized = async (head, inherentProviders, extrinsics, params)=>{
    const registry = await head.registry;
    const header = await newHeader(head);
    const { block: newBlock } = await initNewBlock(head, header, inherentProviders, params);
    const results = [];
    for (const extrinsic of extrinsics){
        newBlock.pushStorageLayer();
        const resp = await newBlock.call('BlockBuilder_apply_extrinsic', [
            extrinsic
        ]);
        const outcome = registry.createType('ApplyExtrinsicResult', resp.result);
        results.push({
            outcome,
            storageDiff: resp.storageDiff
        });
        newBlock.popStorageLayer();
    }
    return results;
};
const dryRunExtrinsic = async (head, inherentProviders, extrinsic, params)=>{
    const registry = await head.registry;
    const header = await newHeader(head);
    const { block: newBlock } = await initNewBlock(head, header, inherentProviders, params);
    if (typeof extrinsic !== 'string') {
        if (!head.chain.mockSignatureHost) {
            throw new Error('Cannot fake signature because mock signature host is not enabled. Start chain with `mockSignatureHost: true`');
        }
        const meta = await head.meta;
        const call = registry.createType('Call', (0, _util.hexToU8a)(extrinsic.call));
        const generic = registry.createType('GenericExtrinsic', call);
        const accountRaw = await head.get((0, _index.compactHex)(meta.query.system.account(extrinsic.address)));
        const account = registry.createType('AccountInfo', (0, _util.hexToU8a)(accountRaw));
        generic.signFake(extrinsic.address, {
            blockHash: head.hash,
            genesisHash: head.hash,
            runtimeVersion: await head.runtimeVersion,
            nonce: account.nonce
        });
        const mockSignature = new Uint8Array(64);
        mockSignature.fill(0xcd);
        mockSignature.set([
            0xde,
            0xad,
            0xbe,
            0xef
        ]);
        generic.signature.set(mockSignature);
        logger.debug({
            call: call.toHuman()
        }, 'dry_run_call');
        return newBlock.call('BlockBuilder_apply_extrinsic', [
            generic.toHex()
        ]);
    }
    logger.debug({
        call: registry.createType('GenericExtrinsic', (0, _util.hexToU8a)(extrinsic)).toJSON()
    }, 'dry_run_extrinsic');
    return newBlock.call('BlockBuilder_apply_extrinsic', [
        extrinsic
    ]);
};
const dryRunInherents = async (head, inherentProviders, params)=>{
    const header = await newHeader(head);
    const { layers } = await initNewBlock(head, header, inherentProviders, params);
    const storage = {};
    for (const layer of layers){
        await layer.mergeInto(storage);
    }
    return Object.entries(storage);
};
