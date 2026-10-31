/* @ts-self-types="./polkadot_wasm_crypto_reviewed.d.ts" */ /**
 * Generate a bip39 phrase
 *
 * words: number of words, either 12, 15, 18 21 or 24
 *
 * Returns the bip 39 phrase
 * @param {number} words
 * @returns {string}
 */ export function ext_bip39_generate(words) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.ext_bip39_generate(words);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally{
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}
/**
 * Create entropy from a bip39 phrase
 *
 * * phrase: mnemonic phrase
 *
 * Returns the entropy
 * @param {string} phrase
 * @returns {Uint8Array}
 */ export function ext_bip39_to_entropy(phrase) {
    const ptr0 = passStringToWasm0(phrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_bip39_to_entropy(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * Create a mini-secret from a bip39 phrase
 *
 * * phrase: mnemonic phrase
 *
 * Returns the 32-byte mini-secret via entropy
 * @param {string} phrase
 * @param {string} password
 * @returns {Uint8Array}
 */ export function ext_bip39_to_mini_secret(phrase, password) {
    const ptr0 = passStringToWasm0(phrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(password, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_bip39_to_mini_secret(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Creates a BTC/ETH compatible seed from a bip-39 phrase
 *
 * @phrase: mnemonic phrase
 *
 * Returns a 32-byte seed
 * @param {string} phrase
 * @param {string} password
 * @returns {Uint8Array}
 */ export function ext_bip39_to_seed(phrase, password) {
    const ptr0 = passStringToWasm0(phrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(password, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_bip39_to_seed(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Validates a bip39 phrase
 *
 * * phrase: mnemonic phrase
 *
 * Returns the true/false
 * @param {string} phrase
 * @returns {boolean}
 */ export function ext_bip39_validate(phrase) {
    const ptr0 = passStringToWasm0(phrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_bip39_validate(ptr0, len0);
    return ret !== 0;
}
/**
 * blake2b hash for the specified input
 *
 * * data: Arbitrary data to be hashed
 * * key: Key to add to the hashing (normally empty)
 * * size: Size in bytes of the resulting output
 *
 * Returns a vector with the hash result
 * @param {Uint8Array} data
 * @param {Uint8Array} key
 * @param {number} size
 * @returns {Uint8Array}
 */ export function ext_blake2b(data, key, size) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(key, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_blake2b(ptr0, len0, ptr1, len1, size);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Generate a key pair.
 *
 * * seed: UIntArray with 32 element
 *
 * returned vector is the concatenation of first the seed (32 bytes)
 * followed by the public key (32) bytes, as the full secret keys.
 * @param {Uint8Array} seed
 * @returns {Uint8Array}
 */ export function ext_ed_from_seed(seed) {
    const ptr0 = passArray8ToWasm0(seed, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_ed_from_seed(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * Sign a message
 *
 * The combination of both public and private key must be provided.
 * This is effectively equivalent to a keypair.
 *
 * * _: UIntArray with 32 element (was pubkey, now ignored)
 * * private: UIntArray with 32 element
 * * message: Arbitrary length UIntArray
 *
 * * returned vector is the signature consisting of 64 bytes.
 * @param {Uint8Array} arg0
 * @param {Uint8Array} seckey
 * @param {Uint8Array} message
 * @returns {Uint8Array}
 */ export function ext_ed_sign(arg0, seckey, message) {
    const ptr0 = passArray8ToWasm0(arg0, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(seckey, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.ext_ed_sign(ptr0, len0, ptr1, len1, ptr2, len2);
    var v4 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v4;
}
/**
 * Verify a message and its corresponding against a public key;
 *
 * * signature: UIntArray with 64 element
 * * message: Arbitrary length UIntArray
 * * pubkey: UIntArray with 32 element
 * @param {Uint8Array} signature
 * @param {Uint8Array} message
 * @param {Uint8Array} pubkey
 * @returns {boolean}
 */ export function ext_ed_verify(signature, message, pubkey) {
    const ptr0 = passArray8ToWasm0(signature, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.ext_ed_verify(ptr0, len0, ptr1, len1, ptr2, len2);
    return ret !== 0;
}
/**
 * hmac with sha256
 * @param {Uint8Array} key
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */ export function ext_hmac_sha256(key, data) {
    const ptr0 = passArray8ToWasm0(key, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_hmac_sha256(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * hmac with sha512
 * @param {Uint8Array} key
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */ export function ext_hmac_sha512(key, data) {
    const ptr0 = passArray8ToWasm0(key, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_hmac_sha512(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Create a keccak256 hash for the specified input
 *
 *
 * Returns the hash as a vector
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */ export function ext_keccak256(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_keccak256(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * Create a keccak512 hash for the specified input
 *
 *
 * Returns the hash as a vector
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */ export function ext_keccak512(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_keccak512(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * pbkdf2 kdf from an input, salt for the number of specified rounds
 *
 * * data: Arbitrary data to be hashed
 * * salt: Salt for this hash
 * * rounds: The number of rounds to perform
 *
 * Returns a vector with the hashed result
 * @param {Uint8Array} data
 * @param {Uint8Array} salt
 * @param {number} rounds
 * @returns {Uint8Array}
 */ export function ext_pbkdf2(data, salt, rounds) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(salt, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_pbkdf2(ptr0, len0, ptr1, len1, rounds);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * scrypt kdf from input, salt and config
 *
 * * password: Password to hash
 * * salt: Salt for this hash
 * * log2_n: log2(n)
 * * r: r
 * * p: p
 *
 * Returns vector with the hashed result
 * @param {Uint8Array} password
 * @param {Uint8Array} salt
 * @param {number} log2_n
 * @param {number} r
 * @param {number} p
 * @returns {Uint8Array}
 */ export function ext_scrypt(password, salt, log2_n, r, p) {
    const ptr0 = passArray8ToWasm0(password, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(salt, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_scrypt(ptr0, len0, ptr1, len1, log2_n, r, p);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * @param {Uint8Array} seed
 * @returns {Uint8Array}
 */ export function ext_secp_from_seed(seed) {
    const ptr0 = passArray8ToWasm0(seed, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_secp_from_seed(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * @param {Uint8Array} pubkey
 * @returns {Uint8Array}
 */ export function ext_secp_pub_compress(pubkey) {
    const ptr0 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_secp_pub_compress(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * @param {Uint8Array} pubkey
 * @returns {Uint8Array}
 */ export function ext_secp_pub_expand(pubkey) {
    const ptr0 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_secp_pub_expand(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * @param {Uint8Array} hash
 * @param {Uint8Array} sig
 * @param {number} rec
 * @returns {Uint8Array}
 */ export function ext_secp_recover(hash, sig, rec) {
    const ptr0 = passArray8ToWasm0(hash, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(sig, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_secp_recover(ptr0, len0, ptr1, len1, rec);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * @param {Uint8Array} hash
 * @param {Uint8Array} seckey
 * @returns {Uint8Array}
 */ export function ext_secp_sign(hash, seckey) {
    const ptr0 = passArray8ToWasm0(hash, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(seckey, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_secp_sign(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * sha256 hash for the specified input
 *
 * * data: Arbitrary data to be hashed
 *
 * Returns a vector with the hash result
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */ export function ext_sha256(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sha256(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * sha512 hash for the specified input
 *
 * * data: Arbitrary data to be hashed
 *
 * Returns a vector with the hash result
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */ export function ext_sha512(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sha512(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * Key agreement between other's public key and self secret key.
 *
 * * pubkey: UIntArray with 32 element
 * * secret: UIntArray with 64 element
 *
 * * returned vector is the generated secret of 32 bytes.
 * @param {Uint8Array} pubkey
 * @param {Uint8Array} secret
 * @returns {Uint8Array}
 */ export function ext_sr_agree(pubkey, secret) {
    const ptr0 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(secret, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_agree(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Perform a derivation on a secret
 *
 * * secret: UIntArray with 64 bytes
 * * cc: UIntArray with 32 bytes
 *
 * returned vector the derived keypair as a array of 96 bytes
 * @param {Uint8Array} pair
 * @param {Uint8Array} cc
 * @returns {Uint8Array}
 */ export function ext_sr_derive_keypair_hard(pair, cc) {
    const ptr0 = passArray8ToWasm0(pair, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(cc, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_derive_keypair_hard(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Perform a derivation on a secret
 *
 * * secret: UIntArray with 64 bytes
 * * cc: UIntArray with 32 bytes
 *
 * returned vector the derived keypair as a array of 96 bytes
 * @param {Uint8Array} pair
 * @param {Uint8Array} cc
 * @returns {Uint8Array}
 */ export function ext_sr_derive_keypair_soft(pair, cc) {
    const ptr0 = passArray8ToWasm0(pair, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(cc, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_derive_keypair_soft(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Perform a derivation on a publicKey
 *
 * * pubkey: UIntArray with 32 bytes
 * * cc: UIntArray with 32 bytes
 *
 * returned vector is the derived publicKey as a array of 32 bytes
 * @param {Uint8Array} pubkey
 * @param {Uint8Array} cc
 * @returns {Uint8Array}
 */ export function ext_sr_derive_public_soft(pubkey, cc) {
    const ptr0 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(cc, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_derive_public_soft(ptr0, len0, ptr1, len1);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}
/**
 * Generate a key pair.
 *
 * * seed: UIntArray with 32 element
 *
 * returned vector is the concatenation of first the private key (64 bytes)
 * followed by the public key (32) bytes.
 * @param {Uint8Array} seed
 * @returns {Uint8Array}
 */ export function ext_sr_from_seed(seed) {
    const ptr0 = passArray8ToWasm0(seed, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_from_seed(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * Sign a message
 *
 * The combination of both public and private key must be provided.
 * This is effectively equivalent to a keypair.
 *
 * * pubkey: UIntArray with 32 element
 * * private: UIntArray with 64 element
 * * message: Arbitrary length UIntArray
 *
 * * returned vector is the signature consisting of 64 bytes.
 * @param {Uint8Array} pubkey
 * @param {Uint8Array} secret
 * @param {Uint8Array} message
 * @returns {Uint8Array}
 */ export function ext_sr_sign(pubkey, secret, message) {
    const ptr0 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(secret, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_sign(ptr0, len0, ptr1, len1, ptr2, len2);
    var v4 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v4;
}
/**
 * Verify a message and its corresponding against a public key;
 *
 * * signature: UIntArray with 64 element
 * * message: Arbitrary length UIntArray
 * * pubkey: UIntArray with 32 element
 * @param {Uint8Array} signature
 * @param {Uint8Array} message
 * @param {Uint8Array} pubkey
 * @returns {boolean}
 */ export function ext_sr_verify(signature, message, pubkey) {
    const ptr0 = passArray8ToWasm0(signature, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.ext_sr_verify(ptr0, len0, ptr1, len1, ptr2, len2);
    return ret !== 0;
}
/**
 * twox hash for the specified input and rounds
 *
 * * data: Arbitrary data to be hashed
 * * rounds: Number of 8-byte rounds to add to the output
 *
 * Returns a vector with the hash result
 * @param {Uint8Array} data
 * @param {number} rounds
 * @returns {Uint8Array}
 */ export function ext_twox(data, rounds) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.ext_twox(ptr0, len0, rounds);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}
/**
 * Run a Random Verifiable Function (VRF) on one single input
 * (message) transcript, and an extra message transcript,
 * producing the output signature and corresponding short proof.
 *
 * * secret: UIntArray with 64 element
 * * context: Arbitrary length UIntArray
 * * message: Arbitrary length UIntArray
 * * extra: Arbitrary length UIntArray
 *
 * * returned vector is the 32-byte output (signature) and 64-byte proof.
 * @param {Uint8Array} secret
 * @param {Uint8Array} ctx
 * @param {Uint8Array} msg
 * @param {Uint8Array} extra
 * @returns {Uint8Array}
 */ export function ext_vrf_sign(secret, ctx, msg, extra) {
    const ptr0 = passArray8ToWasm0(secret, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(ctx, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(msg, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passArray8ToWasm0(extra, wasm.__wbindgen_malloc);
    const len3 = WASM_VECTOR_LEN;
    const ret = wasm.ext_vrf_sign(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
    var v5 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v5;
}
/**
 * Verify VRF proof for one single input transcript, and an extra message transcript,
 * and corresponding output.
 *
 * * pubkey: UIntArray with 32 element
 * * context: Arbitrary length UIntArray
 * * message: Arbitrary length UIntArray
 * * extra: Arbitrary length UIntArray
 * * out_and_proof: 96-byte output & proof array from the ext_sign function.
 * @param {Uint8Array} pubkey
 * @param {Uint8Array} ctx
 * @param {Uint8Array} msg
 * @param {Uint8Array} extra
 * @param {Uint8Array} out
 * @returns {boolean}
 */ export function ext_vrf_verify(pubkey, ctx, msg, extra, out) {
    const ptr0 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(ctx, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(msg, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passArray8ToWasm0(extra, wasm.__wbindgen_malloc);
    const len3 = WASM_VECTOR_LEN;
    const ptr4 = passArray8ToWasm0(out, wasm.__wbindgen_malloc);
    const len4 = WASM_VECTOR_LEN;
    const ret = wasm.ext_vrf_verify(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
    return ret !== 0;
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_is_function_1f9d30630b8b1d3d: function(arg0) {
            const ret = typeof arg0 === 'function';
            return ret;
        },
        __wbg___wbindgen_is_object_3c45d4f2dde4e749: function(arg0) {
            const val = arg0;
            const ret = typeof val === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_string_90b56bc79aad6f6c: function(arg0) {
            const ret = typeof arg0 === 'string';
            return ret;
        },
        __wbg___wbindgen_is_undefined_8865fb403f8fe9d8: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_throw_41e9ee4f547fc59a: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_call_187d372bd5fdd4aa: function() {
            return handleError(function(arg0, arg1, arg2) {
                const ret = arg0.call(arg1, arg2);
                return ret;
            }, arguments);
        },
        __wbg_crypto_38df2bab126b63dc: function(arg0) {
            const ret = arg0.crypto;
            return ret;
        },
        __wbg_getRandomValues_436a51d0629d84e1: function() {
            return handleError(function(arg0, arg1) {
                globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
            }, arguments);
        },
        __wbg_getRandomValues_c44a50d8cfdaebeb: function() {
            return handleError(function(arg0, arg1) {
                arg0.getRandomValues(arg1);
            }, arguments);
        },
        __wbg_length_7f3c00c40364105e: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_msCrypto_bd5a034af96bcba6: function(arg0) {
            const ret = arg0.msCrypto;
            return ret;
        },
        __wbg_new_with_length_3da0ad195f6f63ba: function(arg0) {
            const ret = new Uint8Array(arg0 >>> 0);
            return ret;
        },
        __wbg_node_84ea875411254db1: function(arg0) {
            const ret = arg0.node;
            return ret;
        },
        __wbg_process_44c7a14e11e9f69e: function(arg0) {
            const ret = arg0.process;
            return ret;
        },
        __wbg_prototypesetcall_bc27214492979395: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_randomFillSync_6c25eac9869eb53c: function() {
            return handleError(function(arg0, arg1) {
                arg0.randomFillSync(arg1);
            }, arguments);
        },
        __wbg_require_b4edbdcf3e2a1ef0: function() {
            return handleError(function() {
                const ret = module.require;
                return ret;
            }, arguments);
        },
        __wbg_static_accessor_GLOBAL_266715b9d96ba635: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_10fb7dc1ae063179: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_0b583911f537483a: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_d7f903d1508cbdc4: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_subarray_002b94d5e13d1411: function(arg0, arg1, arg2) {
            const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
            return ret;
        },
        __wbg_versions_276b2795b1c6a219: function(arg0) {
            const ret = arg0.versions;
            return ret;
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        }
    };
    return {
        __proto__: null,
        "./polkadot_wasm_crypto_reviewed_bg.js": import0
    };
}
function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}
function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}
function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}
let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}
function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}
function isLikeNone(x) {
    return x === undefined || x === null;
}
function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}
function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }
    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;
    const mem = getUint8ArrayMemory0();
    let offset = 0;
    for(; offset < len; offset++){
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);
        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }
    WASM_VECTOR_LEN = offset;
    return ptr;
}
let cachedTextDecoder = new TextDecoder('utf-8', {
    ignoreBOM: true,
    fatal: true
});
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', {
            ignoreBOM: true,
            fatal: true
        });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}
const cachedTextEncoder = new TextEncoder();
if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function(arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}
let WASM_VECTOR_LEN = 0;
let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module1) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module1;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}
async function __wbg_load(module1, imports) {
    if (typeof Response === 'function' && module1 instanceof Response) {
        if (!module1.ok) {
            throw new Error(`failed to fetch Wasm: ${module1.status} ${module1.statusText} fetching '${module1.url}'`);
        }
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module1, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module1.type);
                if (validResponse && module1.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
                } else {
                    throw e;
                }
            }
        }
        const bytes = await module1.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module1, imports);
        if (instance instanceof WebAssembly.Instance) {
            return {
                instance,
                module: module1
            };
        } else {
            return instance;
        }
    }
    function expectedResponseType(type) {
        switch(type){
            case 'basic':
            case 'cors':
            case 'default':
                return true;
        }
        return false;
    }
}
function initSync(module1) {
    if (wasm !== undefined) return wasm;
    if (module1 !== undefined) {
        if (Object.getPrototypeOf(module1) === Object.prototype) {
            ({ module: module1 } = module1);
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead');
        }
    }
    const imports = __wbg_get_imports();
    if (!(module1 instanceof WebAssembly.Module)) {
        module1 = new WebAssembly.Module(module1);
    }
    const instance = new WebAssembly.Instance(module1, imports);
    return __wbg_finalize_init(instance, module1);
}
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({ module_or_path } = module_or_path);
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead');
        }
    }
    if (module_or_path === undefined) {
        module_or_path = new URL('polkadot_wasm_crypto_reviewed_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();
    if (typeof module_or_path === 'string' || typeof Request === 'function' && module_or_path instanceof Request || typeof URL === 'function' && module_or_path instanceof URL) {
        module_or_path = fetch(module_or_path);
    }
    const { instance, module: module1 } = await __wbg_load(await module_or_path, imports);
    return __wbg_finalize_init(instance, module1);
}
export { initSync, __wbg_init as default };
