import { bridge, initBridge } from './init.js';
export { packageInfo } from './packageInfo.js';
export { bridge };
function withWasm(name) {
    return (...params) => {
        if (!bridge.wasm) {
            throw new Error('The WASM interface has not been initialized. Ensure that you wait for the initialization Promise with waitReady() from @polkadot/wasm-crypto (or cryptoWaitReady() from @polkadot/util-crypto) before attempting to use WASM-only interfaces.');
        }
        return bridge.wasm[name](...params);
    };
}
export const bip39Generate = withWasm('ext_bip39_generate');
export const bip39ToEntropy = withWasm('ext_bip39_to_entropy');
export const bip39ToMiniSecret = withWasm('ext_bip39_to_mini_secret');
export const bip39ToSeed = withWasm('ext_bip39_to_seed');
export const bip39Validate = withWasm('ext_bip39_validate');
export const ed25519KeypairFromSeed = withWasm('ext_ed_from_seed');
export const ed25519Sign = withWasm('ext_ed_sign');
export const ed25519Verify = withWasm('ext_ed_verify');
export const secp256k1FromSeed = withWasm('ext_secp_from_seed');
export const secp256k1Compress = withWasm('ext_secp_pub_compress');
export const secp256k1Expand = withWasm('ext_secp_pub_expand');
export const secp256k1Recover = withWasm('ext_secp_recover');
export const secp256k1Sign = withWasm('ext_secp_sign');
export const sr25519DeriveKeypairHard = withWasm('ext_sr_derive_keypair_hard');
export const sr25519DeriveKeypairSoft = withWasm('ext_sr_derive_keypair_soft');
export const sr25519DerivePublicSoft = withWasm('ext_sr_derive_public_soft');
export const sr25519KeypairFromSeed = withWasm('ext_sr_from_seed');
export const sr25519Sign = withWasm('ext_sr_sign');
export const sr25519Verify = withWasm('ext_sr_verify');
export const sr25519Agree = withWasm('ext_sr_agree');
export const vrfSign = withWasm('ext_vrf_sign');
export const vrfVerify = withWasm('ext_vrf_verify');
export const blake2b = withWasm('ext_blake2b');
export const hmacSha256 = withWasm('ext_hmac_sha256');
export const hmacSha512 = withWasm('ext_hmac_sha512');
export const keccak256 = withWasm('ext_keccak256');
export const keccak512 = withWasm('ext_keccak512');
export const pbkdf2 = withWasm('ext_pbkdf2');
export const scrypt = withWasm('ext_scrypt');
export const sha256 = withWasm('ext_sha256');
export const sha512 = withWasm('ext_sha512');
export const twox = withWasm('ext_twox');
export function isReady() { return !!bridge.wasm; }
export async function waitReady() { return !!(await initBridge()); }
