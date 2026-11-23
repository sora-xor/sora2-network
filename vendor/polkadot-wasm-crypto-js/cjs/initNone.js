"use strict";
const { initBridge } = require('./init.js');
async function initWasm() {
    await initBridge(() => null);
}
initWasm().catch(() => {});

exports.initWasm = initWasm;
