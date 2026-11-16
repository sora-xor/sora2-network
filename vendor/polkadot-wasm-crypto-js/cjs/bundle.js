"use strict";
const { bridge, initBridge } = require('./init.js');
exports.packageInfo = require('./packageInfo.js').packageInfo;
exports.bridge = bridge;
function withWasm(name) {
    return (...params) => {
        if (!bridge.wasm) {
            throw new Error('The WASM interface has not been initialized. Ensure that you wait for the initialization Promise with waitReady() from @polkadot/wasm-crypto (or cryptoWaitReady() from @polkadot/util-crypto) before attempting to use WASM-only interfaces.');
        }
        return bridge.wasm[name](...params);
    };
}
const bip39Generate = withWasm('ext_bip39_generate');
const bip39ToEntropy = withWasm('ext_bip39_to_entropy');
const bip39ToMiniSecret = withWasm('ext_bip39_to_mini_secret');
const bip39ToSeed = withWasm('ext_bip39_to_seed');
const bip39Validate = withWasm('ext_bip39_validate');
const ed25519KeypairFromSeed = withWasm('ext_ed_from_seed');
const ed25519Sign = withWasm('ext_ed_sign');
const ed25519Verify = withWasm('ext_ed_verify');
const secp256k1FromSeed = withWasm('ext_secp_from_seed');
const secp256k1Compress = withWasm('ext_secp_pub_compress');
const secp256k1Expand = withWasm('ext_secp_pub_expand');
const secp256k1Recover = withWasm('ext_secp_recover');
const secp256k1Sign = withWasm('ext_secp_sign');
const sr25519DeriveKeypairHard = withWasm('ext_sr_derive_keypair_hard');
const sr25519DeriveKeypairSoft = withWasm('ext_sr_derive_keypair_soft');
const sr25519DerivePublicSoft = withWasm('ext_sr_derive_public_soft');
const sr25519KeypairFromSeed = withWasm('ext_sr_from_seed');
const sr25519Sign = withWasm('ext_sr_sign');
const sr25519Verify = withWasm('ext_sr_verify');
const sr25519Agree = withWasm('ext_sr_agree');
const vrfSign = withWasm('ext_vrf_sign');
const vrfVerify = withWasm('ext_vrf_verify');
const blake2b = withWasm('ext_blake2b');
const hmacSha256 = withWasm('ext_hmac_sha256');
const hmacSha512 = withWasm('ext_hmac_sha512');
const keccak256 = withWasm('ext_keccak256');
const keccak512 = withWasm('ext_keccak512');
const pbkdf2 = withWasm('ext_pbkdf2');
const scrypt = withWasm('ext_scrypt');
const sha256 = withWasm('ext_sha256');
const sha512 = withWasm('ext_sha512');
const twox = withWasm('ext_twox');
function isReady() { return !!bridge.wasm; }
async function waitReady() { return !!(await initBridge()); }

exports.bip39Generate = bip39Generate;
exports.bip39ToEntropy = bip39ToEntropy;
exports.bip39ToMiniSecret = bip39ToMiniSecret;
exports.bip39ToSeed = bip39ToSeed;
exports.bip39Validate = bip39Validate;
exports.ed25519KeypairFromSeed = ed25519KeypairFromSeed;
exports.ed25519Sign = ed25519Sign;
exports.ed25519Verify = ed25519Verify;
exports.secp256k1FromSeed = secp256k1FromSeed;
exports.secp256k1Compress = secp256k1Compress;
exports.secp256k1Expand = secp256k1Expand;
exports.secp256k1Recover = secp256k1Recover;
exports.secp256k1Sign = secp256k1Sign;
exports.sr25519DeriveKeypairHard = sr25519DeriveKeypairHard;
exports.sr25519DeriveKeypairSoft = sr25519DeriveKeypairSoft;
exports.sr25519DerivePublicSoft = sr25519DerivePublicSoft;
exports.sr25519KeypairFromSeed = sr25519KeypairFromSeed;
exports.sr25519Sign = sr25519Sign;
exports.sr25519Verify = sr25519Verify;
exports.sr25519Agree = sr25519Agree;
exports.vrfSign = vrfSign;
exports.vrfVerify = vrfVerify;
exports.blake2b = blake2b;
exports.hmacSha256 = hmacSha256;
exports.hmacSha512 = hmacSha512;
exports.keccak256 = keccak256;
exports.keccak512 = keccak512;
exports.pbkdf2 = pbkdf2;
exports.scrypt = scrypt;
exports.sha256 = sha256;
exports.sha512 = sha512;
exports.twox = twox;
exports.isReady = isReady;
exports.waitReady = waitReady;
