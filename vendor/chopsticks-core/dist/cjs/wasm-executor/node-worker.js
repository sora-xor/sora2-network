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
const _nodeworker_threads = /*#__PURE__*/ _interop_require_default(require("node:worker_threads"));
const _comlink = require("comlink");
const _nodeadapter = /*#__PURE__*/ _interop_require_default(require("comlink/dist/umd/node-adapter.js"));
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const startWorker = async ()=>{
    const worker = new _nodeworker_threads.default.Worker(new URL('node-wasm-executor.js', require("url").pathToFileURL(__filename).toString()), {
        name: 'chopsticks-wasm-executor'
    });
    return {
        remote: (0, _comlink.wrap)((0, _nodeadapter.default)(worker)),
        terminate: async ()=>{
            await worker.terminate();
        }
    };
};
