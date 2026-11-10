// SORA Nexus bounded EVM consistency kernel; no consensus-finality or credit authority.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { createRequire } = require('node:module');
const assert = (ok, why) => { if (!ok) throw new Error(why); };
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const dep = JSON.parse(fs.readFileSync(path.join(__dirname, 'dependencies.json'), 'utf8'));
assert(dep.schema === 'sora-nexus-repository-keccak-policy-v1' && dep.package === '@noble/hashes' && dep.version === '1.8.0', 'Keccak dependency policy differs');
const resolver = createRequire(path.resolve(__dirname, dep.resolutionPackage));
const dependencyRoot = path.dirname(fs.realpathSync(resolver.resolve('@noble/hashes/sha3')));
for (const item of dep.files) {
  assert(/^[A-Za-z0-9_.-]+$/.test(item.file), 'dependency file path differs');
  const file = path.join(dependencyRoot, item.file);
  assert(fs.realpathSync(file) === file && !fs.lstatSync(file).isSymbolicLink() && sha(fs.readFileSync(file)) === item.sha256, 'dependency hash changed');
}
const installed = JSON.parse(fs.readFileSync(path.join(dependencyRoot, 'package.json'), 'utf8'));
assert(installed.name === dep.package && installed.version === dep.version, 'installed Keccak package differs');
const { keccak_256 } = createRequire(path.join(dependencyRoot, 'package.json'))('./sha3.js');
for (const file of Object.keys(require.cache).filter(p => p.startsWith(dependencyRoot + path.sep))) {
  assert(dep.files.some(row => path.join(dependencyRoot, row.file) === file), 'unlisted loaded Keccak dependency');
}
const keccak = b => Buffer.from(keccak_256(b));
const hx = b => '0x' + b.toString('hex');
const same = (a, b) => a.equals(b);
const EMPTY = Buffer.alloc(0);
const LIMIT = Object.freeze({ input: 24 * 1024 * 1024, file: 64 * 1024 * 1024, output: 96 * 1024 * 1024, blocks: 128, entries: 16384, totalEntries: 32768, envelope: 2 * 1024 * 1024, nodes: 65536, totalNodes: 524288, logs: 65536 });
function exact(v, keys) {
  assert(v && typeof v === 'object' && !Array.isArray(v) && Object.getPrototypeOf(v) === Object.prototype, 'object required');
  assert(Object.keys(v).sort().join('\0') === keys.slice().sort().join('\0'), 'unexpected object fields');
}
function hex(s, width, max = LIMIT.envelope) {
  assert(typeof s === 'string' && s.length <= 2 + max * 2 && /^0x(?:[0-9a-f]{2})*$/.test(s), 'noncanonical/bounded hex');
  const b = Buffer.from(s.slice(2), 'hex');
  assert(width === undefined || b.length === width, 'wrong byte width');
  return b;
}
function decimal(s) {
  assert(typeof s === 'string' && /^(0|[1-9][0-9]{0,77})$/.test(s), 'canonical decimal required');
  return BigInt(s);
}
function uint(b, max = 32) {
  assert(Buffer.isBuffer(b) && b.length <= max && (b.length === 0 || b[0] !== 0), 'noncanonical/bounded integer');
  return b.length ? BigInt(hx(b)) : 0n;
}
function integer(n) {
  n = BigInt(n); assert(n >= 0n, 'negative integer');
  if (!n) return EMPTY;
  let s = n.toString(16); if (s.length % 2) s = '0' + s;
  return Buffer.from(s, 'hex');
}
function encode(value) {
  const list = Array.isArray(value);
  assert(list || Buffer.isBuffer(value), 'RLP value must be bytes/list');
  const body = list ? Buffer.concat(value.map(encode)) : value;
  if (!list && body.length === 1 && body[0] < 128) return body;
  const prefix = list ? 192 : 128;
  if (body.length < 56) return Buffer.concat([Buffer.from([prefix + body.length]), body]);
  const len = integer(body.length);
  return Buffer.concat([Buffer.from([prefix + 55 + len.length]), len, body]);
}
function decode(raw, budget = { remaining: LIMIT.totalNodes }) {
  assert(Buffer.isBuffer(raw) && raw.length > 0 && raw.length <= LIMIT.envelope, 'RLP byte bound');
  let nodes = 0;
  function item(pos, end, depth) {
    assert(depth <= 16 && ++nodes <= LIMIT.nodes && --budget.remaining >= 0 && pos < end, 'RLP depth/node/truncation bound');
    const first = raw[pos++];
    if (first < 128) return [raw.subarray(pos - 1, pos), pos];
    const list = first >= 192;
    const base = list ? 192 : 128;
    let len = first - base;
    if (len > 55) {
      const count = len - 55;
      assert(count <= 4 && pos + count <= end && raw[pos] !== 0, 'RLP length-of-length');
      len = 0;
      for (let i = 0; i < count; i++) len = len * 256 + raw[pos++];
      assert(len >= 56, 'RLP nonminimal long form');
    }
    assert(len <= end - pos, 'RLP truncated payload');
    const stop = pos + len;
    if (!list) {
      assert(len !== 1 || raw[pos] >= 128, 'RLP nonminimal single byte');
      return [raw.subarray(pos, stop), stop];
    }
    const result = [];
    while (pos < stop) { const r = item(pos, stop, depth + 1); result.push(r[0]); pos = r[1]; }
    return [result, stop];
  }
  const [result, used] = item(0, raw.length, 0);
  assert(used === raw.length && same(encode(result), raw), 'RLP trailing/noncanonical encoding');
  return result;
}
function trieRoot(pairs) {
  assert(Array.isArray(pairs) && pairs.length <= LIMIT.entries, 'trie entry bound');
  const seen = new Set();
  let bytes = 0;
  const rows = pairs.map(([key, value]) => {
    assert(Buffer.isBuffer(key) && key.length <= 64 && Buffer.isBuffer(value) && value.length > 0 && value.length <= LIMIT.envelope, 'trie key/value bound');
    bytes += key.length + value.length;
    assert(bytes <= LIMIT.input, 'trie aggregate byte bound');
    assert(!seen.has(hx(key)), 'duplicate trie key'); seen.add(hx(key));
    return { key: [...key].flatMap(v => [v >> 4, v & 15]), value };
  });
  if (!rows.length) return keccak(encode(EMPTY));
  function compact(ns, leaf) {
    const nibbles = (ns.length % 2 ? [leaf ? 3 : 1] : [leaf ? 2 : 0, 0]).concat(ns);
    return Buffer.from(Array.from({ length: nibbles.length / 2 }, (_, i) => nibbles[i * 2] * 16 + nibbles[i * 2 + 1]));
  }
  function reference(node) { const raw = encode(node); return raw.length < 32 ? node : keccak(raw); }
  function node(rs, offset) {
    if (rs.length === 1) return [compact(rs[0].key.slice(offset), true), rs[0].value];
    let shared = 0;
    while (rs.every(r => offset + shared < r.key.length && r.key[offset + shared] === rs[0].key[offset + shared])) shared++;
    if (shared) return [compact(rs[0].key.slice(offset, offset + shared), false), reference(node(rs, offset + shared))];
    const out = Array.from({ length: 17 }, () => EMPTY);
    for (let n = 0; n < 16; n++) { const group = rs.filter(r => r.key[offset] === n); if (group.length) out[n] = reference(node(group, offset + 1)); }
    const terminal = rs.filter(r => r.key.length === offset);
    assert(terminal.length <= 1, 'trie key collision');
    if (terminal.length) out[16] = terminal[0].value;
    return out;
  }
  return keccak(encode(node(rows, 0)));
}
const indexedRoot = values => trieRoot(values.map((v, i) => [encode(integer(i)), v]));
function width(b, n) { assert(Buffer.isBuffer(b) && b.length === n, 'fixed-width field'); }
function header(raw, budget) {
  const a = decode(raw, budget);
  assert(Array.isArray(a) && [15, 16, 17, 20, 21].includes(a.length) && a.every(Buffer.isBuffer), 'unsupported header layout');
  for (const i of [0, 1, 3, 4, 5, 13]) width(a[i], 32);
  width(a[2], 20); width(a[6], 256); width(a[14], 8);
  assert(a[12].length <= 32, 'Ethereum extraData bound');
  uint(a[7]); for (const i of [8, 9, 10, 11]) uint(a[i], 8);
  if (a.length >= 16) uint(a[15]);
  if (a.length >= 17) width(a[16], 32);
  if (a.length >= 20) { uint(a[17], 8); uint(a[18], 8); width(a[19], 32); }
  if (a.length === 21) width(a[20], 32);
  assert(uint(a[10]) <= uint(a[9]), 'header gasUsed exceeds gasLimit');
  return { hash: keccak(raw), parent: a[0], number: uint(a[8]), timestamp: uint(a[11]), txRoot: a[4], receiptRoot: a[5], bloom: a[6], gas: uint(a[10]) };
}
function envelope(raw, budget) {
  assert(raw.length > 0 && raw.length <= LIMIT.envelope, 'envelope bound');
  if (raw[0] >= 192) return { type: 0, fields: decode(raw, budget) };
  assert([1, 2, 3, 4].includes(raw[0]), 'unsupported typed envelope');
  return { type: raw[0], fields: decode(raw.subarray(1), budget) };
}
function accessList(list) {
  assert(Array.isArray(list), 'access list required');
  for (const row of list) {
    assert(Array.isArray(row) && row.length === 2 && Array.isArray(row[1]), 'access list tuple');
    width(row[0], 20); row[1].forEach(v => width(v, 32));
  }
}
function transaction(raw, chainId, budget) {
  const e = envelope(raw, budget), a = e.fields, type = e.type;
  assert(Array.isArray(a) && a.length === [9, 11, 12, 14, 13][type], 'transaction field count/network wrapper');
  let to, data, gas, signatureChainId = null;
  if (type === 0) {
    for (const i of [0, 1, 2, 4, 6, 7, 8]) uint(a[i]);
    uint(a[0], 8);
    to = a[3]; data = a[5]; gas = uint(a[2], 8);
    const v = uint(a[6]);
    assert(v === 27n || v === 28n || (v >= 35n && (v - 35n) / 2n === chainId), 'legacy chain/v mismatch');
    signatureChainId = v >= 35n ? ((v - 35n) / 2n).toString() : null;
  } else {
    for (let i = 0; i <= (type === 1 ? 3 : 4); i++) uint(a[i]);
    assert(uint(a[0]) === chainId, 'typed chain ID mismatch');
    signatureChainId = uint(a[0]).toString(); uint(a[1], 8);
    if (type > 1) assert(uint(a[2]) <= uint(a[3]), 'priority fee exceeds max fee');
    const ti = type === 1 ? 4 : 5;
    to = a[ti]; uint(a[ti + 1]); data = a[ti + 2]; accessList(a[ti + 3]); gas = uint(a[ti - 1], 8);
    if (type === 3) {
      uint(a[9]); assert(Array.isArray(a[10]) && a[10].length > 0, 'blob versioned hashes required');
      a[10].forEach(v => { width(v, 32); assert(v[0] === 1, 'unsupported blob commitment version'); });
    }
    if (type === 4) {
      assert(Array.isArray(a[9]) && a[9].length > 0, 'authorization list required');
      for (const row of a[9]) {
        assert(Array.isArray(row) && row.length === 6, 'authorization tuple');
        uint(row[0]); width(row[1], 20); uint(row[2], 8); uint(row[3], 1); uint(row[4]); uint(row[5]);
      }
    }
    assert(uint(a[a.length - 3]) <= 1n, 'signature y parity');
    uint(a[a.length - 2]); uint(a[a.length - 1]);
  }
  assert(Buffer.isBuffer(to) && (to.length === 20 || (type < 3 && to.length === 0)), 'transaction destination');
  assert(Buffer.isBuffer(data), 'transaction data');
  assert(gas > 0n, 'transaction gas limit zero');
  const order = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;
  const r = uint(a[a.length - 2]), s = uint(a[a.length - 1]);
  assert(r > 0n && r < order && s > 0n && s <= order / 2n, 'invalid outer signature scalar');
  // Format checks only: no signer recovery or execution/consensus validation.
  return { type, hash: keccak(raw), gas, signatureChainId, destination: hx(to) };
}
function bloom(logs) {
  const out = Buffer.alloc(256);
  for (const log of logs) for (const value of [log[0], ...log[1]]) {
    const h = keccak(value);
    for (let i = 0; i < 6; i += 2) { const bit = (h[i] * 256 + h[i + 1]) & 2047; out[255 - (bit >> 3)] |= 1 << (bit & 7); }
  }
  return out;
}
function receipt(raw, expectedType, budget) {
  const e = envelope(raw, budget), a = e.fields;
  assert(e.type === expectedType && Array.isArray(a) && a.length === 4, 'receipt type/shape mismatch');
  const status = uint(a[0], 1); assert(status === 0n || status === 1n, 'unsupported pre-Byzantium/state-root receipt');
  const gas = uint(a[1], 8); width(a[2], 256);
  assert(Array.isArray(a[3]) && a[3].length <= LIMIT.logs, 'receipt log bound');
  for (const log of a[3]) {
    assert(Array.isArray(log) && log.length === 3 && Array.isArray(log[1]) && log[1].length <= 4 && Buffer.isBuffer(log[2]), 'log shape/topics');
    width(log[0], 20); log[1].forEach(v => width(v, 32));
  }
  assert(status !== 0n || a[3].length === 0, 'failed receipt contains logs');
  assert(same(bloom(a[3]), a[2]), 'receipt bloom differs from actual logs');
  return { type: e.type, status, gas, bloom: a[2], logs: a[3] };
}
function verifyRange(input, suppliedAnchor) {
  exact(input, ['schema', 'declaredChainId', 'fromNumber', 'blocks']);
  exact(suppliedAnchor, ['number', 'hash']);
  assert(input.schema === 'sora-nexus-evm-receipt-range-input-v1', 'input schema');
  const chainId = decimal(input.declaredChainId), from = decimal(input.fromNumber), lastNumber = decimal(suppliedAnchor.number), lastHash = hex(suppliedAnchor.hash, 32);
  assert(chainId > 0n && chainId < (1n << 256n) && from <= lastNumber, 'chain/range bounds');
  assert(Array.isArray(input.blocks) && input.blocks.length > 0 && input.blocks.length <= LIMIT.blocks && lastNumber - from + 1n === BigInt(input.blocks.length), 'complete bounded range required');
  let prior = null, totalBytes = 0, totalLogs = 0, totalEntries = 0;
  const budget = { remaining: LIMIT.totalNodes };
  function admittedHex(value, max = LIMIT.envelope) {
    assert(typeof value === "string" && value.length <= 2 + max * 2, "envelope byte bound");
    const bytes = (value.length - 2) / 2;
    assert(bytes >= 0 && totalBytes + bytes <= LIMIT.input, "aggregate input byte bound");
    const result = hex(value, undefined, max); totalBytes += result.length; return result;
  }
  const outputs = [], allLogs = [], seenTx = new Set();
  for (let index = 0; index < input.blocks.length; index++) {
    const item = input.blocks[index]; exact(item, ['header', 'transactions', 'receipts']);
    assert(Array.isArray(item.transactions) && Array.isArray(item.receipts) && item.transactions.length === item.receipts.length && item.transactions.length <= LIMIT.entries, 'complete transaction/receipt count');
    totalEntries += item.transactions.length;
    assert(totalEntries <= LIMIT.totalEntries, "aggregate transaction count");
    const rawHeader = admittedHex(item.header, 4096), h = header(rawHeader, budget);
    assert(h.number === from + BigInt(index), 'header number gap/reorder');
    if (prior) assert(same(h.parent, prior.hash) && h.timestamp > prior.timestamp, 'header ancestry/timestamp mismatch');
    const rawTx = item.transactions.map(v => admittedHex(v)), rawReceipts = item.receipts.map(v => admittedHex(v));
    assert(same(indexedRoot(rawTx), h.txRoot), 'complete transactionsRoot mismatch');
    assert(same(indexedRoot(rawReceipts), h.receiptRoot), 'complete receiptsRoot mismatch');
    let cumulative = 0n, logIndex = 0;
    const blockBloom = Buffer.alloc(256);
    for (let txIndex = 0; txIndex < rawTx.length; txIndex++) {
      const t = transaction(rawTx[txIndex], chainId, budget), r = receipt(rawReceipts[txIndex], t.type, budget);
      assert(!seenTx.has(hx(t.hash)), 'duplicate transaction hash'); seenTx.add(hx(t.hash));
      assert(r.gas > cumulative && r.gas - cumulative <= t.gas, 'cumulative gas order/transaction limit');
      cumulative = r.gas;
      for (let b = 0; b < 256; b++) blockBloom[b] |= r.bloom[b];
      assert(totalLogs + r.logs.length <= LIMIT.logs, 'aggregate log bound'); totalLogs += r.logs.length;
      for (let local = 0; local < r.logs.length; local++) {
        const [address, topics, data] = r.logs[local];
        allLogs.push({ blockNumber: h.number.toString(), blockHash: hx(h.hash), transactionIndex: txIndex, transactionHash: hx(t.hash), transactionType: t.type, signatureChainId: t.signatureChainId, transactionDestination: t.destination, receiptLogIndex: local, logIndex: logIndex++, address: hx(address), topics: topics.map(hx), data: hx(data), status: Number(r.status) });
      }
    }
    assert(cumulative === h.gas, 'last receipt cumulative gas/header gasUsed mismatch');
    assert(same(blockBloom, h.bloom), 'header bloom differs from receipt logs');
    outputs.push({ number: h.number.toString(), hash: hx(h.hash), parentHash: hx(h.parent), transactionsRoot: hx(h.txRoot), receiptsRoot: hx(h.receiptRoot), transactions: rawTx.length, logs: logIndex });
    prior = h;
  }
  assert(prior.number === lastNumber && same(prior.hash, lastHash), 'supplied anchor hash/number mismatch');
  return { schema: 'sora-nexus-evm-receipt-range-consistency-v1', candidateOnly: true, suppliedAnchor, declaredChainId: input.declaredChainId, fromNumber: input.fromNumber, blocks: outputs, logs: allLogs, decodedInputBytes: totalBytes, decodedRlpNodes: LIMIT.totalNodes - budget.remaining,
    headerAncestryToSuppliedAnchorVerified: true, completeTransactionTriesVerified: true, completeReceiptTriesVerified: true, transactionHashInclusionVerified: true, bloomAndGasConsistencyVerified: true, actualReceiptLogsOnly: true,
    externalFinalityVerified: false, trustAnchorApproved: false, sourceChainIdentityVerified: false, globalNetworkCompletenessVerified: false, requiredRangeCoverageVerified: false, transactionSignaturesVerified: false, executionValidityVerified: false, senderAttributionVerified: false, nativeHashiAssociationVerified: false, productionReady: false };
}
module.exports = { LIMIT, hex, integer, uint, encode, decode, keccak, hx, trieRoot, indexedRoot, header, envelope, transaction, receipt, bloom, verifyRange };
