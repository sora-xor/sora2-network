"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "ChopsticksProvider", {
    enumerable: true,
    get: function() {
        return ChopsticksProvider;
    }
});
const _eventemitter3 = require("eventemitter3");
const _logger = require("./logger.js");
const _index = require("./rpc/index.js");
const _setup = require("./setup.js");
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
const providerHandlers = {
    ..._index.allHandlers
};
const logger = _logger.defaultLogger.child({
    name: '[Chopsticks provider]'
});
var _isConnected = /*#__PURE__*/ new WeakMap(), _eventemitter = /*#__PURE__*/ new WeakMap(), _isReadyPromise = /*#__PURE__*/ new WeakMap(), _subscriptions = /*#__PURE__*/ new WeakMap();
class ChopsticksProvider {
    get hasSubscriptions() {
        return true;
    }
    get isClonable() {
        return true;
    }
    get isConnected() {
        return _class_private_field_get(this, _isConnected);
    }
    get isReady() {
        return _class_private_field_get(this, _isReadyPromise);
    }
    subscribe(type, method, params, callback) {
        return this.send(method, params, false, {
            callback,
            type
        });
    }
    async unsubscribe(_type, method, id) {
        if (!_class_private_field_get(this, _subscriptions)[id]) {
            logger.trace(`Unable to find active subscription=${id}`);
            return false;
        }
        try {
            return this.send(method, [
                id
            ]);
        } catch  {
            return false;
        }
    }
    constructor(chain){
        _define_property(this, "chain", void 0);
        _class_private_field_init(this, _isConnected, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _eventemitter, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _isReadyPromise, {
            writable: true,
            value: void 0
        });
        _class_private_field_init(this, _subscriptions, {
            writable: true,
            value: void 0
        });
        _define_property(this, "clone", void 0);
        _define_property(this, "connect", void 0);
        _define_property(this, "disconnect", void 0);
        _define_property(this, "on", void 0);
        _define_property(this, "subscriptionManager", void 0);
        _define_property(this, "send", void 0);
        this.chain = chain;
        _class_private_field_set(this, _isConnected, false);
        _class_private_field_set(this, _eventemitter, new _eventemitter3.EventEmitter());
        _class_private_field_set(this, _subscriptions, {});
        this.clone = ()=>{
            return new ChopsticksProvider(this.chain);
        };
        this.connect = async ()=>{
            _class_private_field_set(this, _isConnected, true);
            _class_private_field_get(this, _eventemitter).emit('connected');
        };
        this.disconnect = async ()=>{
            _class_private_field_set(this, _isConnected, false);
            _class_private_field_get(this, _eventemitter).emit('disconnected');
        };
        this.on = (type, sub)=>{
            _class_private_field_get(this, _eventemitter).on(type, sub);
            return ()=>{
                _class_private_field_get(this, _eventemitter).removeListener(type, sub);
            };
        };
        this.subscriptionManager = {
            subscribe: (method, subid, onCancel = ()=>{})=>{
                const sub = _class_private_field_get(this, _subscriptions)[subid];
                if (sub) {
                    sub.onCancel = onCancel;
                }
                return (data)=>{
                    logger.debug({
                        method,
                        subid,
                        data
                    }, 'subscribe-callback');
                    const sub = _class_private_field_get(this, _subscriptions)[subid];
                    if (sub) {
                        sub.callback(null, data);
                    } else {
                        logger.trace(`Unable to find active subscription=${subid}`);
                    }
                };
            },
            unsubscribe: (subid)=>{
                logger.debug({
                    subid
                }, 'unsubscribe-callback');
                const sub = _class_private_field_get(this, _subscriptions)[subid];
                if (sub) {
                    sub.onCancel?.();
                    delete _class_private_field_get(this, _subscriptions)[subid];
                } else {
                    logger.trace(`Unable to find active subscription=${subid}`);
                }
            }
        };
        this.send = async (method, params, _isCacheable, subscription)=>{
            try {
                logger.debug({
                    method,
                    params
                }, 'send');
                const rpcHandler = providerHandlers[method];
                if (!rpcHandler) {
                    logger.error(`Unable to find rpc handler=${method}`);
                    throw new Error(`Unable to find rpc handler=${method}`);
                }
                if (subscription) {
                    logger.debug({
                        method,
                        params
                    }, 'subscribe');
                    const subid = await rpcHandler({
                        chain: this.chain
                    }, params, this.subscriptionManager);
                    if (!subid) {
                        throw new Error(`Unable to subscribe=${method}`);
                    }
                    _class_private_field_get(this, _subscriptions)[subid] = {
                        callback: subscription.callback,
                        method,
                        params,
                        type: subscription.type
                    };
                    return subid;
                }
                logger.debug({
                    method,
                    params
                }, 'call');
                return rpcHandler({
                    chain: this.chain
                }, params, this.subscriptionManager);
            } catch (e) {
                logger.error({
                    err: e
                }, 'send error');
                throw e;
            }
        };
        _class_private_field_set(this, _isReadyPromise, new Promise((resolve, reject)=>{
            _class_private_field_get(this, _eventemitter).once('connected', resolve);
            _class_private_field_get(this, _eventemitter).once('error', reject);
            this.connect();
        }));
    }
}
_define_property(ChopsticksProvider, "fromEndpoint", async (endpoint, block, db)=>{
    return new ChopsticksProvider(await (0, _setup.setup)({
        endpoint,
        mockSignatureHost: true,
        block,
        db
    }));
});
