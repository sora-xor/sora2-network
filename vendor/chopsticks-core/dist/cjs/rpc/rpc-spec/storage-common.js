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
    get PAGE_SIZE () {
        return PAGE_SIZE;
    },
    get afterResponse () {
        return afterResponse;
    },
    get getDescendantValues () {
        return getDescendantValues;
    }
});
async function getDescendantValues(block, params) {
    const keys = await block.getKeysPaged({
        ...params,
        pageSize: PAGE_SIZE
    });
    const items = (await block.getMany(keys)).map((value, idx)=>({
            key: keys[idx],
            value
        }));
    if (keys.length < PAGE_SIZE) {
        return {
            items,
            next: null
        };
    }
    return {
        items,
        next: {
            ...params,
            startKey: keys[PAGE_SIZE - 1]
        }
    };
}
const PAGE_SIZE = 1000;
async function afterResponse(fn) {
    await new Promise((resolve)=>setTimeout(resolve, 0));
    fn();
}
