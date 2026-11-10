import { initBridge } from './init.js';
import { createWasm } from '@polkadot/wasm-crypto-wasm';
export async function initWasm() {
    await initBridge(createWasm);
}
initWasm().catch(() => {});
