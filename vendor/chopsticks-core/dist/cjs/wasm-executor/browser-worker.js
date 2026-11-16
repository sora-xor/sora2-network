"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "startWorker", {
    enumerable: true,
    get: function() {
        return startWorker;
    }
});
const _comlink = require("comlink");
const startWorker = async ()=>{
    const worker = new Worker(new URL('browser-wasm-executor.js', require("url").pathToFileURL(__filename).toString()), {
        type: 'module',
        name: 'chopsticks-wasm-executor'
    });
    return {
        remote: (0, _comlink.wrap)(worker),
        terminate: async ()=>{
            worker.terminate();
        }
    };
};
