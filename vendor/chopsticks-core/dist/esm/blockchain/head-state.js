import { defaultLogger } from '../logger.js';
export const randomId = ()=>Math.random().toString(36).substring(2);
const logger = defaultLogger.child({
    name: 'head-state'
});
export class HeadState {
    #headListeners = {};
    #storageListeners = {};
    #oldValues = {};
    #head;
    constructor(head){
        this.#head = head;
    }
    subscribeHead(cb) {
        const id = randomId();
        this.#headListeners[id] = cb;
        return id;
    }
    unsubscribeHead(id) {
        delete this.#headListeners[id];
    }
    async subscribeStorage(keys, cb) {
        const id = randomId();
        this.#storageListeners[id] = [
            keys,
            cb
        ];
        for (const key of keys){
            this.#oldValues[key] = await this.#head.get(key).then((val)=>val || null);
        }
        return id;
    }
    unsubscribeStorage(id) {
        delete this.#storageListeners[id];
    }
    async setHead(head) {
        this.#head = head;
        for (const cb of Object.values(this.#headListeners)){
            try {
                await cb(head);
            } catch (error) {
                logger.error(error, 'setHead head callback error');
            }
        }
        const diff = await this.#head.storageDiff();
        const newValues = {
            ...diff
        };
        const watchedKeys = new Set();
        for (const [keys] of Object.values(this.#storageListeners)){
            for (const key of keys)watchedKeys.add(key);
        }
        for (const key of watchedKeys){
            if (newValues[key] === undefined && this.#oldValues[key] !== undefined) {
                newValues[key] = await head.get(key) ?? null;
            }
        }
        for (const [keys, cb] of Object.values(this.#storageListeners)){
            const changes = [];
            for (const key of keys){
                if (newValues[key] === undefined) continue;
                if (newValues[key] !== this.#oldValues[key]) {
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
            this.#oldValues[key] = value;
        }
    }
}
