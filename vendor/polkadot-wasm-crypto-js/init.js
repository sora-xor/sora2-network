// The generated wasm-bindgen wrappers own allocation and return-value decoding.
// Legacy pointer glue cannot be mixed with the rebuilt module's modern ABI.
import { createWasm } from '@polkadot/wasm-crypto-wasm';
let pending;
export const bridge = {
    wasm: null,
    type: 'none',
    error: null,
    async init(factory) {
        if (!pending || factory) {
            pending = Promise.resolve().then(() => (factory || createWasm)()).then((wasm) => {
                if (wasm && typeof wasm.ext_sha256 !== 'function') {
                    throw new Error('A reviewed wasm-bindgen crypto module is required.');
                }
                bridge.wasm = wasm || null;
                bridge.type = wasm ? 'wasm' : 'none';
                bridge.error = null;
                return bridge.wasm;
            }).catch((error) => {
                bridge.wasm = null;
                bridge.type = 'none';
                bridge.error = String(error.message || error);
                return null;
            });
        }
        return pending;
    }
};
export async function initBridge(factory) {
    return bridge.init(factory);
}
