"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "default", {
    enumerable: true,
    get: function() {
        return KeyCache;
    }
});
const _lodash = /*#__PURE__*/ _interop_require_default(require("lodash"));
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
class KeyCache {
    feed(keys) {
        const _keys = keys.filter((key)=>key.length >= this.prefixLength);
        if (_keys.length === 0) return;
        const startKey = _keys[0].slice(this.prefixLength);
        const endKey = _keys[_keys.length - 1].slice(this.prefixLength);
        const grouped = _lodash.default.groupBy(_keys, (key)=>key.slice(0, this.prefixLength));
        for (const [prefix, keys] of Object.entries(grouped)){
            const ranges = this.ranges.filter((range)=>range.prefix === prefix);
            if (ranges.length === 0) {
                // no existing range with prefix
                this.ranges.push({
                    prefix,
                    keys: keys.map((i)=>i.slice(this.prefixLength))
                });
                continue;
            }
            let merged = false;
            for (const range of ranges){
                const startPosition = _lodash.default.sortedIndex(range.keys, startKey);
                if (startPosition >= 0 && range.keys[startPosition] === startKey) {
                    // found existing range with prefix
                    range.keys.splice(startPosition, keys.length, ...keys.map((i)=>i.slice(this.prefixLength)));
                    merged = true;
                    break;
                }
                const endPosition = _lodash.default.sortedIndex(range.keys, endKey);
                if (endPosition >= 0 && range.keys[endPosition] === endKey) {
                    // found existing range with prefix
                    range.keys.splice(0, endPosition + 1, ...keys.map((i)=>i.slice(this.prefixLength)));
                    merged = true;
                    break;
                }
            }
            // insert new prefix with range
            if (!merged) {
                this.ranges.push({
                    prefix,
                    keys: keys.map((i)=>i.slice(this.prefixLength))
                });
            }
        }
    // TODO: merge ranges if they overlap
    }
    async next(startKey) {
        if (startKey.length < this.prefixLength) return;
        const prefix = startKey.slice(0, this.prefixLength);
        const key = startKey.slice(this.prefixLength);
        for (const range of this.ranges.filter((range)=>range.prefix === prefix)){
            if (key.length === 0) {
                // if key is empty then find the range with first key empty
                if (range.keys[0] !== '') continue;
                return [
                    prefix,
                    range.keys[1]
                ].join('');
            }
            const index = _lodash.default.sortedIndex(range.keys, key);
            if (range.keys[index] !== key) continue;
            const nextKey = range.keys[index + 1];
            if (nextKey) {
                return [
                    prefix,
                    nextKey
                ].join('');
            }
        }
    }
    constructor(prefixLength){
        _define_property(this, "prefixLength", void 0);
        _define_property(this, "ranges", void 0);
        this.prefixLength = prefixLength;
        this.ranges = [];
    }
}
