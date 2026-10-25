import { wrap } from 'comlink';
export const startWorker = async ()=>{
    const worker = new Worker(new URL('browser-wasm-executor.js', import.meta.url), {
        type: 'module',
        name: 'chopsticks-wasm-executor'
    });
    return {
        remote: wrap(worker),
        terminate: async ()=>{
            worker.terminate();
        }
    };
};
