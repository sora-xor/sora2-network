"use strict";
const { initBridge } = require('./init.js');
async function initWasm() {
    await initBridge(() => { throw new Error('asm.js crypto is no longer supported; use a WebAssembly-capable runtime.'); });
}
initWasm().catch(() => {});

exports.initWasm = initWasm;
