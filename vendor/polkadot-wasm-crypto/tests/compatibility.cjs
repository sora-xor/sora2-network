#!/usr/bin/env node
'use strict';

// Public synthetic fixtures recorded from the published @polkadot/wasm-crypto
// 7.5.4 bundle. The retired dependency graph is only used when explicitly passed
// as --record-reference/--reference; normal candidate checks use inert fixtures.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');

const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, item, index, args) => {
  if (index % 2 === 0) pairs.push([item, args[index + 1]]);
  return pairs;
}, []));
const fixturePath = path.resolve(options['--fixtures'] || path.join(__dirname, 'fixtures/wasm-crypto-7.5.4.json'));
const hex = (value) => Buffer.from(value).toString('hex');
const bytes = (value) => Uint8Array.from(Buffer.from(value, 'hex'));
const utf8 = (value) => Uint8Array.from(Buffer.from(value));
const derive = (label, length) => {
  const parts = [];
  for (let counter = 0; Buffer.concat(parts).length < length; counter++) {
    parts.push(crypto.createHash('sha512').update(`SORA Nexus public crypto fixture/${label}/${counter}`).digest());
  }
  return Uint8Array.from(Buffer.concat(parts).subarray(0, length));
};
const encode = (value) => value instanceof Uint8Array ? { bytes: hex(value) } : value;
const decode = (value) => value && typeof value === 'object' && 'bytes' in value ? bytes(value.bytes) : value;
const order = BigInt('0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141');
const scalarBytes = (n) => bytes(n.toString(16).padStart(64, '0'));
const mutate = (value, offset = 0) => { const result = Uint8Array.from(value); result[offset] ^= 1; return result; };
const alterMessage = (value) => Uint8Array.from([...value, 1]);
const stableSoftPair = (pair) => Uint8Array.from([...pair.subarray(0, 32), ...pair.subarray(64)]);
const evidenceByApi = new WeakMap();

async function load(packagePath) {
  const location = path.resolve(packagePath);
  const request = createRequire(path.join(location, 'package.json'));
  let api = request(location);
  const wasmPackage = api.createWasm ? api : request('@polkadot/wasm-crypto-wasm');
  if (api.createWasm && !api.waitReady) {
    const raw = await api.createWasm();
    const names = {
      bip39Generate: 'bip39_generate', bip39ToEntropy: 'bip39_to_entropy', bip39ToMiniSecret: 'bip39_to_mini_secret', bip39ToSeed: 'bip39_to_seed', bip39Validate: 'bip39_validate',
      ed25519KeypairFromSeed: 'ed_from_seed', ed25519Sign: 'ed_sign', ed25519Verify: 'ed_verify',
      secp256k1FromSeed: 'secp_from_seed', secp256k1Compress: 'secp_pub_compress', secp256k1Expand: 'secp_pub_expand', secp256k1Recover: 'secp_recover', secp256k1Sign: 'secp_sign',
      sr25519DeriveKeypairHard: 'sr_derive_keypair_hard', sr25519DeriveKeypairSoft: 'sr_derive_keypair_soft', sr25519DerivePublicSoft: 'sr_derive_public_soft', sr25519KeypairFromSeed: 'sr_from_seed', sr25519Sign: 'sr_sign', sr25519Verify: 'sr_verify', sr25519Agree: 'sr_agree',
      vrfSign: 'vrf_sign', vrfVerify: 'vrf_verify', blake2b: 'blake2b', hmacSha256: 'hmac_sha256', hmacSha512: 'hmac_sha512', keccak256: 'keccak256', keccak512: 'keccak512', pbkdf2: 'pbkdf2', scrypt: 'scrypt', sha256: 'sha256', sha512: 'sha512', twox: 'twox',
    };
    api = Object.fromEntries(Object.entries(names).map(([name, suffix]) => [name, raw['ext_' + suffix]]));
    api.waitReady = async () => true;
  }
  assert.equal(await api.waitReady(), true, `Wasm initialization: ${location}`);
  evidenceByApi.set(api, {
    wasm_sha256: crypto.createHash('sha256').update(wasmPackage.wasmBytes).digest('hex'),
    wasm_bytes: wasmPackage.wasmBytes.length,
  });
  return api;
}

async function record(referencePath) {
  const api = await load(referencePath);
  const fixture = { version: 1, source: '@polkadot/wasm-crypto 7.5.4 published npm Wasm', purpose: 'Public synthetic compatibility vectors; never use these keys for funds.', inputs: [], deterministic: [], signing: [], derivations: [], mnemonics: [] };
  const referenceRequire = createRequire(path.join(path.resolve(referencePath), 'package.json'));
  const referenceWasm = referenceRequire('@polkadot/wasm-crypto-wasm').wasmBytes;
  fixture.reference_wasm_sha256 = crypto.createHash('sha256').update(referenceWasm).digest('hex');
  fixture.reference_wasm_bytes = referenceWasm.length;
  const inputIndex = new Map();
  function argument(value) {
    if (!(value instanceof Uint8Array)) return value;
    const valueHex = hex(value);
    if (!inputIndex.has(valueHex)) { inputIndex.set(valueHex, fixture.inputs.length); fixture.inputs.push(valueHex); }
    return { input: inputIndex.get(valueHex) };
  }
  function add(label, fn, args, transform = null) {
    let result = api[fn](...args);
    if (transform === 'stableSoftPair') result = stableSoftPair(result);
    fixture.deterministic.push({ label, fn, args: args.map(argument), result: encode(result), ...(transform && { transform }) });
    return result;
  }
  const phrases = [
    'abandon '.repeat(11) + 'about',
    'legal winner thank year wave sausage worth useful legal winner thank yellow',
    'zoo '.repeat(23) + 'vote',
    ...[15, 18, 21].map((words) => api.bip39Generate(words)),
  ];
  for (const [i, phrase] of phrases.entries()) {
    for (const [variant, text] of Object.entries({ plain: phrase, surrounding: ` ${phrase} `, doubled: phrase.replaceAll(' ', '  '), tabs: phrase.replaceAll(' ', '\t'), ideographic: phrase.replaceAll(' ', '\u3000'), uppercase: phrase.toUpperCase() })) {
      const valid = add(`mnemonic/${i}/${variant}/validate`, 'bip39Validate', [text]);
      if (valid || ['surrounding', 'doubled', 'tabs', 'ideographic'].includes(variant)) {
        add(`mnemonic/${i}/${variant}/entropy`, 'bip39ToEntropy', [text]);
        for (const password of ['', 'Substrate', 'caf\u00e9', 'cafe\u0301', '\u334d\u30ac\u30d0\u30f4\u30a1\u3071\u3070\u3050\u309e\u3061\u3062\u5341\u4eba\u5341\u8272']) {
          add(`mnemonic/${i}/${variant}/seed/${password}`, 'bip39ToSeed', [text, password]);
          add(`mnemonic/${i}/${variant}/mini/${password}`, 'bip39ToMiniSecret', [text, password]);
        }
      }
    }
    fixture.mnemonics.push(phrase);
  }
  add('mnemonic/invalid-checksum', 'bip39Validate', ['abandon '.repeat(11) + 'abandon']);
  for (const size of [0, 1, 7, 31, 32, 33, 63, 64, 65, 255, 1024, 4096, 65536]) {
    const data = derive(`hash/${size}`, size), key = derive(`key/${size}`, size % 65);
    for (const fn of ['sha256', 'sha512', 'keccak256', 'keccak512']) add(`hash/${size}/${fn}`, fn, [data]);
    for (const fn of ['hmacSha256', 'hmacSha512']) add(`hash/${size}/${fn}`, fn, [key, data]);
    for (const output of [1, 16, 20, 32, 48, 64]) add(`hash/${size}/blake2b/${output}`, 'blake2b', [data, key, output]);
    for (const rounds of [1, 2, 4]) add(`hash/${size}/twox/${rounds}`, 'twox', [data, rounds]);
  }
  for (const rounds of [1, 2, 17, 2048]) add(`pbkdf2/${rounds}`, 'pbkdf2', [utf8('password\0caf\u00e9'), utf8('salt\0'), rounds]);
  for (const [log2n, r, p] of [[1, 1, 1], [4, 1, 1], [10, 8, 1], [8, 2, 2]]) add(`scrypt/${log2n}/${r}/${p}`, 'scrypt', [utf8('password\0'), utf8('NaCl'), log2n, r, p]);
  const messages = [0, 1, 31, 32, 255, 4096].map((size) => derive(`message/${size}`, size));
  const digests = [0n, 1n, order - 1n, order, order + 1n, (1n << 256n) - 1n].map(scalarBytes);
  for (let i = 0; i < 16; i++) {
    const seed = i === 0 ? new Uint8Array(32) : derive(`seed/${i}`, 32);
    const secpSeed = Uint8Array.from(seed); secpSeed[0] &= 0x7f; secpSeed[31] |= 1;
    const ed = add(`key/${i}/ed`, 'ed25519KeypairFromSeed', [seed]);
    const sr = add(`key/${i}/sr`, 'sr25519KeypairFromSeed', [seed]);
    const secp = add(`key/${i}/secp`, 'secp256k1FromSeed', [secpSeed]);
    const expanded = add(`key/${i}/secp-expand`, 'secp256k1Expand', [secp.subarray(32)]);
    add(`key/${i}/secp-compress`, 'secp256k1Compress', [expanded]);
    const cc = derive(`chaincode/${i}`, 32);
    const hard = add(`key/${i}/sr-hard`, 'sr25519DeriveKeypairHard', [sr, cc]);
    const soft = api.sr25519DeriveKeypairSoft(sr, cc);
    add(`key/${i}/sr-soft`, 'sr25519DeriveKeypairSoft', [sr, cc], 'stableSoftPair');
    add(`key/${i}/sr-public-soft`, 'sr25519DerivePublicSoft', [sr.subarray(64), cc]);
    fixture.derivations.push({ pair: hex(sr), cc: hex(cc), public: hex(soft.subarray(64)), hard: hex(hard) });
    const other = api.sr25519KeypairFromSeed(derive(`peer/${i}`, 32));
    add(`key/${i}/agreement`, 'sr25519Agree', [other.subarray(64), sr.subarray(0, 64)]);
    for (const [j, message] of messages.entries()) {
      const label = `sign/${i}/${j}`;
      add(`${label}/ed`, 'ed25519Sign', [ed.subarray(32), ed.subarray(0, 32), message]);
      add(`${label}/ed-ignores-untrusted-public`, 'ed25519Sign', [other.subarray(64), ed.subarray(0, 32), message]);
      const hash = j < digests.length ? digests[j] : api.sha256(message);
      const signature = add(`${label}/secp`, 'secp256k1Sign', [hash, secpSeed]);
      add(`${label}/secp-recover`, 'secp256k1Recover', [hash, signature.subarray(0, 64), signature[64]]);
      const context = utf8(j % 2 ? 'substrate' : ''), extra = derive(`extra/${j}`, j * 7);
      fixture.signing.push({ label, message: hex(message), ed: hex(ed), sr: hex(sr), secpSeed: hex(secpSeed), hash: hex(hash), secpSignature: hex(signature), srSignature: hex(api.sr25519Sign(sr.subarray(64), sr.subarray(0, 64), message)), context: hex(context), extra: hex(extra), vrf: hex(api.vrfSign(sr.subarray(0, 64), context, message, extra)) });
    }
  }
  fs.writeFileSync(fixturePath, JSON.stringify(fixture, null, 2) + '\n');
  console.log(JSON.stringify({ recorded: fixturePath, deterministic: fixture.deterministic.length, signing: fixture.signing.length, sha256: crypto.createHash('sha256').update(fs.readFileSync(fixturePath)).digest('hex') }));
}

async function check(candidatePath, referencePath) {
  const api = await load(candidatePath), old = referencePath ? await load(referencePath) : null;
  const fixture = JSON.parse(fs.readFileSync(fixturePath));
  if (old) assert.equal(evidenceByApi.get(old).wasm_sha256, fixture.reference_wasm_sha256, 'Reference Wasm matches frozen fixture provenance');
  let assertions = 0;
  const failures = [];
  const coverage = new Set();
  function equal(actual, expected, label) {
    assertions++;
    try { assert.deepEqual(encode(actual), encode(expected), label); } catch (error) {
      if (options['--collect-failures'] !== 'true') throw error;
      failures.push({ label, actual: encode(actual), expected: encode(expected) });
    }
  }
  for (const item of fixture.deterministic) {
    coverage.add(item.fn);
    let result = api[item.fn](...item.args.map((value) => value && typeof value === 'object' && 'input' in value ? bytes(fixture.inputs[value.input]) : decode(value)));
    if (item.transform === 'stableSoftPair') result = stableSoftPair(result);
    equal(result, decode(item.result), item.label);
  }
  for (const words of [12, 15, 18, 21, 24]) {
    const phrase = api.bip39Generate(words);
    coverage.add('bip39Generate');
    equal(phrase.split(' ').length, words, `generated/${words}/length`);
    equal(api.bip39Validate(phrase), true, `generated/${words}/valid`);
    if (old) equal(old.bip39Validate(phrase), true, `generated/${words}/old-valid`);
  }
  for (const item of fixture.derivations) {
    const pair = bytes(item.pair), cc = bytes(item.cc);
    const soft = api.sr25519DeriveKeypairSoft(pair, cc);
    equal(soft.subarray(64), bytes(item.public), 'soft-derived public');
    const sig = api.sr25519Sign(soft.subarray(64), soft.subarray(0, 64), cc);
    equal(api.sr25519Verify(sig, cc, soft.subarray(64)), true, 'soft-derived signature');
    if (old) equal(old.sr25519Verify(sig, cc, bytes(item.public)), true, 'old verifies derived signature');
  }
  for (const item of fixture.signing) {
    const message = bytes(item.message), ed = bytes(item.ed), sr = bytes(item.sr);
    const pub = sr.subarray(64), secret = sr.subarray(0, 64), context = bytes(item.context), extra = bytes(item.extra);
    coverage.add('ed25519Verify'); coverage.add('sr25519Verify'); coverage.add('sr25519Sign'); coverage.add('vrfSign'); coverage.add('vrfVerify');
    const edSig = api.ed25519Sign(ed.subarray(32), ed.subarray(0, 32), message);
    equal(api.ed25519Verify(edSig, message, ed.subarray(32)), true, `${item.label}/ed-verify`);
    equal(api.ed25519Verify(edSig, alterMessage(message), ed.subarray(32)), false, `${item.label}/ed-tampered-message`);
    const oldSig = bytes(item.srSignature), sig = api.sr25519Sign(pub, secret, message);
    equal(api.sr25519Verify(oldSig, message, pub), true, `${item.label}/new-verifies-old-sr`);
    equal(api.sr25519Verify(sig, message, pub), true, `${item.label}/sr-verify`);
    equal(api.sr25519Verify(sig, alterMessage(message), pub), false, `${item.label}/sr-tampered-message`);
    equal(api.sr25519Verify(mutate(sig), message, pub), false, `${item.label}/sr-tampered-signature`);
    const proof = api.vrfSign(secret, context, message, extra), oldProof = bytes(item.vrf);
    equal(proof.subarray(0, 32), oldProof.subarray(0, 32), `${item.label}/vrf-output`);
    equal(api.vrfVerify(pub, context, message, extra, oldProof), true, `${item.label}/new-verifies-old-vrf`);
    equal(api.vrfVerify(pub, context, message, extra, proof), true, `${item.label}/vrf-verify`);
    equal(api.vrfVerify(pub, context, alterMessage(message), extra, proof), false, `${item.label}/vrf-tampered-message`);
    equal(api.vrfVerify(pub, alterMessage(context), message, extra, proof), false, `${item.label}/vrf-tampered-context`);
    equal(api.vrfVerify(pub, context, message, alterMessage(extra), proof), false, `${item.label}/vrf-tampered-extra`);
    equal(api.vrfVerify(pub, context, message, extra, mutate(proof)), false, `${item.label}/vrf-tampered-output`);
    equal(api.vrfVerify(pub, context, message, extra, mutate(proof, 32)), false, `${item.label}/vrf-tampered-proof`);
    equal(api.vrfVerify(pub, context, message, extra, Uint8Array.from([...proof, 1, 2])), true, `${item.label}/vrf-trailing-compatibility`);
    if (old) {
      equal(old.ed25519Verify(edSig, message, ed.subarray(32)), true, `${item.label}/old-verifies-new-ed`);
      equal(old.sr25519Verify(sig, message, pub), true, `${item.label}/old-verifies-new-sr`);
      equal(old.vrfVerify(pub, context, message, extra, proof), true, `${item.label}/old-verifies-new-vrf`);
    }
  }
  // Recovery must preserve the published wrapper's strict low-S boundary.
  for (const item of fixture.signing.slice(0, 6)) {
    const signature = bytes(item.secpSignature), high = Uint8Array.from(signature.subarray(0, 64));
    const s = BigInt('0x' + hex(high.subarray(32)));
    high.set(scalarBytes(order - s), 32);
    assert.throws(() => api.secp256k1Recover(bytes(item.hash), high, signature[64] ^ 1), undefined, `${item.label}/high-S`); assertions++;
  }
  const sample = fixture.signing[0], edPair = bytes(sample.ed), srPair = bytes(sample.sr), message = bytes(sample.message);
  const edSignature = api.ed25519Sign(edPair.subarray(32), edPair.subarray(0, 32), message);
  const srSignature = bytes(sample.srSignature);
  for (const length of [0, 1, 31, 63, 65]) {
    equal(api.ed25519Verify(new Uint8Array(length), message, edPair.subarray(32)), false, `Ed signature length/${length}`);
    equal(api.sr25519Verify(new Uint8Array(length), message, srPair.subarray(64)), false, `SR signature length/${length}`);
  }
  for (const length of [0, 1, 31, 33]) {
    equal(api.ed25519Verify(edSignature, message, new Uint8Array(length)), false, `Ed public length/${length}`);
    equal(api.sr25519Verify(srSignature, message, new Uint8Array(length)), false, `SR public length/${length}`);
  }
  for (const length of [0, 1, 31, 33]) {
    assert.throws(() => api.secp256k1Recover(new Uint8Array(length), bytes(sample.secpSignature).subarray(0, 64), 0), undefined, `secp digest length/${length}`); assertions++;
  }
  for (const length of [0, 1, 63, 65]) {
    assert.throws(() => api.secp256k1Recover(bytes(sample.hash), new Uint8Array(length), 0), undefined, `secp signature length/${length}`); assertions++;
  }
  if (options['--security-regressions'] === 'true') {
    const item = fixture.signing[0], sr = bytes(item.sr);
    assert.throws(() => api.sr25519Sign(bytes(fixture.signing[6].sr).subarray(64), sr.subarray(0, 64), bytes(item.message))); assertions++;
    for (const length of [0, 1, 31, 32, 63, 95]) {
      equal(api.vrfVerify(sr.subarray(64), bytes(item.context), bytes(item.message), bytes(item.extra), new Uint8Array(length)), false, `short VRF/${length}`);
    }
  }
  const result = { candidate: path.resolve(candidatePath), candidate_artifact: evidenceByApi.get(api), reference: referencePath ? path.resolve(referencePath) : null, reference_artifact: old ? evidenceByApi.get(old) : null, node_version: process.version, node_platform: process.platform, node_architecture: process.arch, fixture_sha256: crypto.createHash('sha256').update(fs.readFileSync(fixturePath)).digest('hex'), deterministic_vectors: fixture.deterministic.length, signing_vectors: fixture.signing.length, assertions, failures, exported_functions_covered: [...coverage].sort(), scope: 'Synthetic keys; no network, RPC, transaction submission, or deployment.' };
  console.log(JSON.stringify(result, null, 2));
  if (failures.length) process.exitCode = 1;
}

(async () => {
  if (options['--record-reference']) await record(options['--record-reference']);
  else {
    assert(options['--candidate'], 'Pass --candidate <package directory>, or --record-reference <isolated published package directory>');
    await check(options['--candidate'], options['--reference']);
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
