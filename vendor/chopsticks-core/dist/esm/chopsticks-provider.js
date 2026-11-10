import { EventEmitter } from 'eventemitter3';
import { defaultLogger } from './logger.js';
import { allHandlers } from './rpc/index.js';
import { setup } from './setup.js';
const providerHandlers = {
    ...allHandlers
};
const logger = defaultLogger.child({
    name: '[Chopsticks provider]'
});
/**
 * Provider for local chopsticks chain
 */ export class ChopsticksProvider {
    chain;
    #isConnected = false;
    #eventemitter = new EventEmitter();
    #isReadyPromise;
    #subscriptions = {};
    constructor(chain){
        this.chain = chain;
        this.#isReadyPromise = new Promise((resolve, reject)=>{
            this.#eventemitter.once('connected', resolve);
            this.#eventemitter.once('error', reject);
            this.connect();
        });
    }
    static fromEndpoint = async (endpoint, block, db)=>{
        return new ChopsticksProvider(await setup({
            endpoint,
            mockSignatureHost: true,
            block,
            db
        }));
    };
    get hasSubscriptions() {
        return true;
    }
    get isClonable() {
        return true;
    }
    get isConnected() {
        return this.#isConnected;
    }
    get isReady() {
        return this.#isReadyPromise;
    }
    clone = ()=>{
        return new ChopsticksProvider(this.chain);
    };
    connect = async ()=>{
        this.#isConnected = true;
        this.#eventemitter.emit('connected');
    };
    disconnect = async ()=>{
        this.#isConnected = false;
        this.#eventemitter.emit('disconnected');
    };
    on = (type, sub)=>{
        this.#eventemitter.on(type, sub);
        return ()=>{
            this.#eventemitter.removeListener(type, sub);
        };
    };
    subscriptionManager = {
        subscribe: (method, subid, onCancel = ()=>{})=>{
            const sub = this.#subscriptions[subid];
            if (sub) {
                sub.onCancel = onCancel;
            }
            return (data)=>{
                logger.debug({
                    method,
                    subid,
                    data
                }, 'subscribe-callback');
                const sub = this.#subscriptions[subid];
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
            const sub = this.#subscriptions[subid];
            if (sub) {
                sub.onCancel?.();
                delete this.#subscriptions[subid];
            } else {
                logger.trace(`Unable to find active subscription=${subid}`);
            }
        }
    };
    send = async (method, params, _isCacheable, subscription)=>{
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
                this.#subscriptions[subid] = {
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
    subscribe(type, method, params, callback) {
        return this.send(method, params, false, {
            callback,
            type
        });
    }
    async unsubscribe(_type, method, id) {
        if (!this.#subscriptions[id]) {
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
}
