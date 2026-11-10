"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "decodeWellKnownKey", {
    enumerable: true,
    get: function() {
        return decodeWellKnownKey;
    }
});
const _util = require("@polkadot/util");
const _utilcrypto = require("@polkadot/util-crypto");
const decodeValue = (type)=>(registry, value)=>{
        return registry.createType(type, (0, _util.hexToU8a)(value)).toJSON();
    };
// https://github.com/paritytech/polkadot-sdk/issues/2126
const wellKnownKeys = [
    {
        name: 'code',
        key: ':code',
        decodeValue: (_registry, value)=>{
            return `<:code blake2_256 ${(0, _utilcrypto.blake2AsHex)(value, 256)} (${value.length / 2 - 1} bytes)>`;
        }
    },
    {
        name: 'heapPages',
        key: ':heappages',
        type: 'u64'
    },
    {
        name: 'extrinsicIndex',
        key: ':extrinsic_index',
        type: 'u32'
    },
    {
        name: 'intrablockEntropy',
        key: ':intrablock_entropy',
        type: '[u8; 32]'
    },
    {
        name: 'transactionLevel',
        key: ':transaction_level:',
        type: 'u32'
    },
    {
        name: 'grandpaAuthorities',
        key: ':grandpa_authorities',
        type: '(u8, AuthorityList)'
    },
    {
        name: 'relayDispatchQueueRemainingCapacity',
        prefix: ':relay_dispatch_queue_remaining_capacity',
        decodeKey: (registry, key)=>{
            return [
                registry.createType('u32', (0, _util.hexToU8a)(key)).toJSON()
            ];
        },
        type: '(u32, u32)'
    },
    {
        name: 'relay_dispatch_queue_size',
        prefixHex: '0xf5207f03cfdce586301014700e2c2593fad157e461d71fd4c1f936839a5f1f3e',
        decodeKey: (registry, key)=>{
            // get last 4 bytes
            const last4Bytes = `0x${key.slice(-8)}`;
            return [
                registry.createType('u32', (0, _util.hexToU8a)(last4Bytes)).toJSON()
            ];
        },
        type: '(u32, u32)'
    }
].map((def)=>{
    const prefix = def.prefixHex ?? (0, _util.stringToHex)(def.prefix || def.key);
    return {
        name: def.name,
        prefix,
        decodeKey: def.decodeKey || ((_registry, key)=>[
                key
            ]),
        decodeValue: def.decodeValue || decodeValue(def.type)
    };
});
const decodeWellKnownKey = (registry, key, value)=>{
    for (const defs of wellKnownKeys){
        if (key.startsWith(defs.prefix)) {
            const remaining = key.slice(defs.prefix.length);
            const decodedKey = remaining ? defs.decodeKey(registry, `0x${remaining}`) : undefined;
            const decodedValue = value ? defs.decodeValue(registry, value) : undefined;
            return {
                name: defs.name,
                key: decodedKey ?? [],
                value: decodedValue
            };
        }
    }
};
