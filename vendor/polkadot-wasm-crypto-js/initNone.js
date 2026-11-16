import { initBridge } from './init.js';
export async function initWasm() {
    await initBridge(() => null);
}
initWasm().catch(() => {});
