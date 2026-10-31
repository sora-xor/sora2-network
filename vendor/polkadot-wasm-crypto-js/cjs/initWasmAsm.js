"use strict";
const { initBridge } = require('./init.js');
const { createWasm } = require('@polkadot/wasm-crypto-wasm');
async function initWasm() {
    await initBridge(createWasm);
}
initWasm().catch(() => {});

exports.initWasm = initWasm;
