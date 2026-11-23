/* tslint:disable */
/* eslint-disable */

/**
 * Generate a bip39 phrase
 *
 * words: number of words, either 12, 15, 18 21 or 24
 *
 * Returns the bip 39 phrase
 */
export function ext_bip39_generate(words: number): string;

/**
 * Create entropy from a bip39 phrase
 *
 * * phrase: mnemonic phrase
 *
 * Returns the entropy
 */
export function ext_bip39_to_entropy(phrase: string): Uint8Array;

/**
 * Create a mini-secret from a bip39 phrase
 *
 * * phrase: mnemonic phrase
 *
 * Returns the 32-byte mini-secret via entropy
 */
export function ext_bip39_to_mini_secret(phrase: string, password: string): Uint8Array;

/**
 * Creates a BTC/ETH compatible seed from a bip-39 phrase
 *
 * @phrase: mnemonic phrase
 *
 * Returns a 32-byte seed
 */
export function ext_bip39_to_seed(phrase: string, password: string): Uint8Array;

/**
 * Validates a bip39 phrase
 *
 * * phrase: mnemonic phrase
 *
 * Returns the true/false
 */
export function ext_bip39_validate(phrase: string): boolean;

/**
 * blake2b hash for the specified input
 *
 * * data: Arbitrary data to be hashed
 * * key: Key to add to the hashing (normally empty)
 * * size: Size in bytes of the resulting output
 *
 * Returns a vector with the hash result
 */
export function ext_blake2b(data: Uint8Array, key: Uint8Array, size: number): Uint8Array;

/**
 * Generate a key pair.
 *
 * * seed: UIntArray with 32 element
 *
 * returned vector is the concatenation of first the seed (32 bytes)
 * followed by the public key (32) bytes, as the full secret keys.
 */
export function ext_ed_from_seed(seed: Uint8Array): Uint8Array;

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
 */
export function ext_ed_sign(arg0: Uint8Array, seckey: Uint8Array, message: Uint8Array): Uint8Array;

/**
 * Verify a message and its corresponding against a public key;
 *
 * * signature: UIntArray with 64 element
 * * message: Arbitrary length UIntArray
 * * pubkey: UIntArray with 32 element
 */
export function ext_ed_verify(signature: Uint8Array, message: Uint8Array, pubkey: Uint8Array): boolean;

/**
 * hmac with sha256
 */
export function ext_hmac_sha256(key: Uint8Array, data: Uint8Array): Uint8Array;

/**
 * hmac with sha512
 */
export function ext_hmac_sha512(key: Uint8Array, data: Uint8Array): Uint8Array;

/**
 * Create a keccak256 hash for the specified input
 *
 *
 * Returns the hash as a vector
 */
export function ext_keccak256(data: Uint8Array): Uint8Array;

/**
 * Create a keccak512 hash for the specified input
 *
 *
 * Returns the hash as a vector
 */
export function ext_keccak512(data: Uint8Array): Uint8Array;

/**
 * pbkdf2 kdf from an input, salt for the number of specified rounds
 *
 * * data: Arbitrary data to be hashed
 * * salt: Salt for this hash
 * * rounds: The number of rounds to perform
 *
 * Returns a vector with the hashed result
 */
export function ext_pbkdf2(data: Uint8Array, salt: Uint8Array, rounds: number): Uint8Array;

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
 */
export function ext_scrypt(password: Uint8Array, salt: Uint8Array, log2_n: number, r: number, p: number): Uint8Array;

export function ext_secp_from_seed(seed: Uint8Array): Uint8Array;

export function ext_secp_pub_compress(pubkey: Uint8Array): Uint8Array;

export function ext_secp_pub_expand(pubkey: Uint8Array): Uint8Array;

export function ext_secp_recover(hash: Uint8Array, sig: Uint8Array, rec: number): Uint8Array;

export function ext_secp_sign(hash: Uint8Array, seckey: Uint8Array): Uint8Array;

/**
 * sha256 hash for the specified input
 *
 * * data: Arbitrary data to be hashed
 *
 * Returns a vector with the hash result
 */
export function ext_sha256(data: Uint8Array): Uint8Array;

/**
 * sha512 hash for the specified input
 *
 * * data: Arbitrary data to be hashed
 *
 * Returns a vector with the hash result
 */
export function ext_sha512(data: Uint8Array): Uint8Array;

/**
 * Key agreement between other's public key and self secret key.
 *
 * * pubkey: UIntArray with 32 element
 * * secret: UIntArray with 64 element
 *
 * * returned vector is the generated secret of 32 bytes.
 */
export function ext_sr_agree(pubkey: Uint8Array, secret: Uint8Array): Uint8Array;

/**
 * Perform a derivation on a secret
 *
 * * secret: UIntArray with 64 bytes
 * * cc: UIntArray with 32 bytes
 *
 * returned vector the derived keypair as a array of 96 bytes
 */
export function ext_sr_derive_keypair_hard(pair: Uint8Array, cc: Uint8Array): Uint8Array;

/**
 * Perform a derivation on a secret
 *
 * * secret: UIntArray with 64 bytes
 * * cc: UIntArray with 32 bytes
 *
 * returned vector the derived keypair as a array of 96 bytes
 */
export function ext_sr_derive_keypair_soft(pair: Uint8Array, cc: Uint8Array): Uint8Array;

/**
 * Perform a derivation on a publicKey
 *
 * * pubkey: UIntArray with 32 bytes
 * * cc: UIntArray with 32 bytes
 *
 * returned vector is the derived publicKey as a array of 32 bytes
 */
export function ext_sr_derive_public_soft(pubkey: Uint8Array, cc: Uint8Array): Uint8Array;

/**
 * Generate a key pair.
 *
 * * seed: UIntArray with 32 element
 *
 * returned vector is the concatenation of first the private key (64 bytes)
 * followed by the public key (32) bytes.
 */
export function ext_sr_from_seed(seed: Uint8Array): Uint8Array;

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
 */
export function ext_sr_sign(pubkey: Uint8Array, secret: Uint8Array, message: Uint8Array): Uint8Array;

/**
 * Verify a message and its corresponding against a public key;
 *
 * * signature: UIntArray with 64 element
 * * message: Arbitrary length UIntArray
 * * pubkey: UIntArray with 32 element
 */
export function ext_sr_verify(signature: Uint8Array, message: Uint8Array, pubkey: Uint8Array): boolean;

/**
 * twox hash for the specified input and rounds
 *
 * * data: Arbitrary data to be hashed
 * * rounds: Number of 8-byte rounds to add to the output
 *
 * Returns a vector with the hash result
 */
export function ext_twox(data: Uint8Array, rounds: number): Uint8Array;

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
 */
export function ext_vrf_sign(secret: Uint8Array, ctx: Uint8Array, msg: Uint8Array, extra: Uint8Array): Uint8Array;

/**
 * Verify VRF proof for one single input transcript, and an extra message transcript,
 * and corresponding output.
 *
 * * pubkey: UIntArray with 32 element
 * * context: Arbitrary length UIntArray
 * * message: Arbitrary length UIntArray
 * * extra: Arbitrary length UIntArray
 * * out_and_proof: 96-byte output & proof array from the ext_sign function.
 */
export function ext_vrf_verify(pubkey: Uint8Array, ctx: Uint8Array, msg: Uint8Array, extra: Uint8Array, out: Uint8Array): boolean;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly ext_bip39_generate: (a: number) => [number, number];
    readonly ext_bip39_to_entropy: (a: number, b: number) => [number, number];
    readonly ext_bip39_to_mini_secret: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_bip39_to_seed: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_bip39_validate: (a: number, b: number) => number;
    readonly ext_blake2b: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly ext_ed_from_seed: (a: number, b: number) => [number, number];
    readonly ext_ed_sign: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly ext_ed_verify: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly ext_hmac_sha256: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_hmac_sha512: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_keccak256: (a: number, b: number) => [number, number];
    readonly ext_keccak512: (a: number, b: number) => [number, number];
    readonly ext_pbkdf2: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly ext_scrypt: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly ext_secp_from_seed: (a: number, b: number) => [number, number];
    readonly ext_secp_pub_compress: (a: number, b: number) => [number, number];
    readonly ext_secp_pub_expand: (a: number, b: number) => [number, number];
    readonly ext_secp_recover: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly ext_secp_sign: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_sha256: (a: number, b: number) => [number, number];
    readonly ext_sha512: (a: number, b: number) => [number, number];
    readonly ext_sr_agree: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_sr_derive_keypair_hard: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_sr_derive_keypair_soft: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_sr_derive_public_soft: (a: number, b: number, c: number, d: number) => [number, number];
    readonly ext_sr_from_seed: (a: number, b: number) => [number, number];
    readonly ext_sr_sign: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly ext_sr_verify: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly ext_twox: (a: number, b: number, c: number) => [number, number];
    readonly ext_vrf_sign: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly ext_vrf_verify: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
