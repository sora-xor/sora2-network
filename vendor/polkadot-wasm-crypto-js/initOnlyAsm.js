import { initBridge } from './init.js';
export async function initWasm() {
    await initBridge(() => { throw new Error('asm.js crypto is no longer supported; use a WebAssembly-capable runtime.'); });
}
initWasm().catch(() => {});
