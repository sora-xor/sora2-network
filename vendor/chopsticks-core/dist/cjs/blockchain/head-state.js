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
    get HeadState () {
        return HeadState;
    },
    get randomId () {
        return randomId;
    }
});
const _logger = require("../logger.js");
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
const randomId = ()=>Math.random().toString(36).substring(2);
const logger = _logger.defaultLogger.child({
    name: 'head-state'
});
var _headListeners = /*#__PURE__*/ new WeakMap(), _storageListeners = /*#__PURE__*/ new WeakMap(), _oldValues = /*#__PURE__*/ new WeakMap(), _head = /*#__PURE__*/ new WeakMap();
class HeadState {
    subscribeHead(cb) {
        const id = randomId();
        _class_private_field_get(this, _headListeners)[id] = cb;
        return id;
    }
    unsubscribeHead(id) {
        delete _class_private_field_get(this, _headListeners)[id];
    }
    async subscribeStorage(keys, cb) {
        const id = randomId();
        _class_private_field_get(this, _storageListeners)[id] = [
            keys,
            cb
        ];
        for (const key of keys){
            _class_private_field_get(this, _oldValues)[key] = await _class_private_field_get(this, _head).get(key).then((val)=>val || null);
        }
        return id;
    }
    unsubscribeStorage(id) {
        delete _class_private_field_get(this, _storageListeners)[id];
    }
    async setHead(head) {
        _class_private_field_set(this, _head, head);
        for (const cb of Object.values(_class_private_field_get(this, _headListeners))){
            try {
                await cb(head);
            } catch (error) {
                logger.error(error, 'setHead head callback error');
            }
        }
        const diff = await _class_private_field_get(this, _head).storageDiff();
        const newValues = {
            ...diff
        };
        const watchedKeys = new Set();
        for (const [keys] of Object.values(_class_private_field_get(this, _storageListeners))){
            for (const key of keys)watchedKeys.add(key);
        }
        for (const key of watchedKeys){
            if (newValues[key] === undefined && _class_private_field_get(this, _oldValues)[key] !== undefined) {
                newValues[key] = await head.get(key) ?? null;
            }
        }
        for (const [keys, cb] of Object.values(_class_private_field_get(this, _storageListeners))){
            const changes = [];
            for (const key of keys){
                if (newValues[key] === undefined) continue;
                if (newValues[key] !== _class_private_field_get(this, _oldValues)[key]) {
                    changes.push([
                        key,
                        newValues[key]
                    ]);
                }
            }
            if (changes.length > 0) {
                try {
                    await cb(head, changes);
                } catch (error) {
                    logger.error(error, 'setHead storage diff callback error');
                }
            }
        }
        for (const [key, value] of Object.entries(newValues)){
            _class_private_field_get(this, _oldValues)[key] = value;
        }
    }
    constructor(head){
        _class_private_field_init(this, _headListeners, {
            writable: true,
            value: {}
        });
        _class_private_field_init(this, _storageListeners, {
            writable: true,
            value: {}
        });
        _class_private_field_init(this, _oldValues, {
            writable: true,
            value: {}
        });
        _class_private_field_init(this, _head, {
            writable: true,
            value: void 0
        });
        _class_private_field_set(this, _head, head);
    }
}
