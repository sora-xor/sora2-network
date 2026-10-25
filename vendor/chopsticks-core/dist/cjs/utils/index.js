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
    get CHILD_PREFIX_LENGTH () {
        return CHILD_PREFIX_LENGTH;
    },
    get PREFIX_LENGTH () {
        return PREFIX_LENGTH;
    },
    get compactHex () {
        return compactHex;
    },
    get defer () {
        return defer;
    },
    get fetchKeys () {
        return fetchKeys;
    },
    get fetchKeysToArray () {
        return fetchKeysToArray;
    },
    get getCurrentSlot () {
        return getCurrentSlot;
    },
    get getCurrentTimestamp () {
        return getCurrentTimestamp;
    },
    get getParaId () {
        return getParaId;
    },
    get getSlotDuration () {
        return getSlotDuration;
    },
    get isPrefixedChildKey () {
        return isPrefixedChildKey;
    },
    get isUrl () {
        return isUrl;
    },
    get prefixedChildKey () {
        return prefixedChildKey;
    },
    get splitChildKey () {
        return splitChildKey;
    },
    get stripChildPrefix () {
        return stripChildPrefix;
    }
});
const _util = require("@polkadot/util");
const _hex = require("@polkadot/util/hex");
const _index = require("../wasm-executor/index.js");
_export_star(require("./decoder.js"), exports);
_export_star(require("./set-storage.js"), exports);
_export_star(require("./time-travel.js"), exports);
function _export_star(from, to) {
    Object.keys(from).forEach(function(k) {
        if (k !== "default" && !Object.prototype.hasOwnProperty.call(to, k)) {
            Object.defineProperty(to, k, {
                enumerable: true,
                get: function() {
                    return from[k];
                }
            });
        }
    });
    return from;
}
async function fetchKeys(getKeys, processKey) {
    const processKeys = async (keys)=>{
        for (const key of keys){
            await processKey(key);
        }
        if (keys.length > 0) {
            return keys[keys.length - 1];
        }
        return undefined;
    };
    const keys = await getKeys();
    let nextKey = await processKeys(keys);
    while(nextKey){
        const keys = await getKeys(nextKey.toHex());
        nextKey = await processKeys(keys);
    }
}
async function fetchKeysToArray(getKeys) {
    const res = [];
    await fetchKeys(getKeys, (key)=>res.push(key));
    return res;
}
const compactHex = (value)=>{
    return (0, _util.u8aToHex)((0, _util.compactStripLength)(value)[1]);
};
const getParaId = async (chain)=>{
    const meta = await chain.head.meta;
    const id = await chain.head.read('u32', meta.query.parachainInfo.parachainId);
    if (!id) {
        throw new Error('Cannot find parachain id');
    }
    return id;
};
const isUrl = (url)=>{
    try {
        new URL(url);
        return true;
    } catch (_e) {
        return false;
    }
};
function defer() {
    const deferred = {};
    deferred.promise = new Promise((resolve, reject)=>{
        deferred.resolve = resolve;
        deferred.reject = reject;
    });
    return deferred;
}
// Chopsticks treats both main storage and child storage as a key-value store
// The difference is that child storage keys are prefixed with the child storage key
// :child_storage:default: as hex string
const DEFAULT_CHILD_STORAGE = '0x3a6368696c645f73746f726167653a64656661756c743a';
const CHILD_PREFIX_LENGTH = DEFAULT_CHILD_STORAGE.length + 64;
const PREFIX_LENGTH = 66;
const prefixedChildKey = (prefix, key)=>prefix + (0, _hex.hexStripPrefix)(key);
const isPrefixedChildKey = (key)=>key.startsWith(DEFAULT_CHILD_STORAGE);
const splitChildKey = (key)=>{
    if (!key.startsWith(DEFAULT_CHILD_STORAGE)) return [];
    if (key.length < CHILD_PREFIX_LENGTH) return [];
    const child = key.slice(0, CHILD_PREFIX_LENGTH);
    const rest = key.slice(CHILD_PREFIX_LENGTH);
    return [
        child,
        (0, _hex.hexAddPrefix)(rest)
    ];
};
const stripChildPrefix = (key)=>{
    const [child, storageKey] = splitChildKey(key);
    if (!child) return key;
    return storageKey;
};
const getCurrentSlot = async (head)=>{
    const timestamp = await getCurrentTimestamp(head);
    const slotDuration = await getSlotDuration(head);
    return Math.floor(Number(timestamp / BigInt(slotDuration)));
};
const getCurrentTimestamp = async (head)=>{
    const meta = await head.meta;
    const timestamp = await head.read('u64', meta.query.timestamp.now);
    return timestamp?.toBigInt() ?? BigInt(Date.now());
};
const getSlotDuration = async (head)=>{
    const meta = await head.meta;
    let slotDuration;
    slotDuration ??= meta.consts.babe?.expectedBlockTime?.toNumber();
    slotDuration ??= meta.consts.asyncBacking?.expectedBlockTime?.toNumber();
    return slotDuration || (0, _index.getAuraSlotDuration)(await head.wasm).catch(()=>12_000);
};
