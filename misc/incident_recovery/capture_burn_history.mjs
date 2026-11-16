#!/usr/bin/env node
/** Immutable historical event proofs. This does not assert completeness or
 * decode events. The offline Rust verifier proves every included value/root. */
import {mkdir, readFile, stat} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {gzipSync, gunzipSync} from 'node:zlib';
import {spawn} from 'node:child_process';
import {CHECKPOINT, DEFAULT_ENDPOINT, ReadOnlyRpc, sha256, writeAtomic} from './capture_checkpoint.mjs';

export const FIRST_BURN_BLOCK = 25_043_003;
export const EVENTS_KEY = '0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7';
const CODE_KEY = '0x3a636f6465';
// A raw ':' lookup supplies changing branches above the cached code leaf.
// Its value/absence is never used as evidence that code stayed unchanged.
export const CODE_PREFIX_KEY = '0x3a';
const PAGE_SIZE = 16;
const MAX_PAGE_BYTES = 64 * 1024 * 1024;
const json = value => Buffer.from(JSON.stringify(value) + '\n');
const assert = (value, message) => {if (!value) throw new Error(message);};
const isHash = value => typeof value === 'string' && /^0x[0-9a-f]{64}$/.test(value);
const blob = value => {
  assert(typeof value === 'string' && /^0x(?:[0-9a-f]{2})+$/.test(value), 'Invalid proof node hex');
  const bytes = Buffer.from(value.slice(2), 'hex');
  assert(bytes.length <= 16 * 1024 * 1024, 'Proof node exceeds capture bound');
  return bytes;
};
async function immutable(path, bytes) {
  try {assert((await readFile(path)).equals(bytes), `Existing immutable capture differs: ${path}`);}
  catch (error) {if (error.code !== 'ENOENT') throw error; await writeAtomic(path, bytes);}
}
export function validateProof(response, hash) {
  assert(response?.at === hash && Array.isArray(response.proof) && response.proof.length > 0 && response.proof.length <= 4096,
    'Missing or wrong-block storage proof');
  const nodes = response.proof.map(blob);
  assert(nodes.reduce((sum, node) => sum + node.length, 0) <= 32 * 1024 * 1024, 'Storage proof exceeds capture bound');
  assert(new Set(nodes.map(sha256)).size === nodes.length, 'Duplicate proof node');
  return nodes;
}
async function proofObjects(output, response, hash) {
  const nodes = validateProof(response, hash);
  const references = [];
  for (const node of nodes) {
    const digest = sha256(node);
    await immutable(join(output, 'objects', digest), node);
    references.push(digest);
  }
  return references;
}
export function validateBlock(block, number, hash, previousHash) {
  assert(isHash(hash) && block?.block?.header && Array.isArray(block.block.extrinsics) &&
    Number(BigInt(block.block.header.number)) === number && block.block.header.parentHash === previousHash,
  'Nonconsecutive historical block or missing extrinsics');
  assert(json(block).length <= 16 * 1024 * 1024, 'Historical block exceeds capture bound');
}
/** One pinned native process, bounded replies, and no shell or network access.
 * It retains only authenticated :code-path nodes; the final verifier still
 * reads every explicit per-block proof from the original stream format. */
export async function nativeProofSession(verifier, expectedSha256, output) {
  assert(typeof expectedSha256 === 'string' && /^[0-9a-f]{64}$/.test(expectedSha256), 'Native verifier SHA-256 is required');
  const binary = resolve(verifier);
  assert(sha256(await readFile(binary)) === expectedSha256, 'Native proof verifier SHA-256 mismatch');
  const child = spawn(binary, ['burn-proof-session', '--capture', output], {stdio:['pipe','pipe','pipe']});
  let pending = null, buffer = '', stderr = '', failure = null, nextId = 1, closed = false;
  const fail = error => {failure = error; if (pending) {clearTimeout(pending.timer); pending.reject(error); pending = null;}};
  const exited = new Promise(resolveExit => {
    child.once('error', error => {fail(error); resolveExit();});
    child.once('exit', (code, signal) => {
      closed = true;
      if (code !== 0 || pending || buffer) fail(new Error(`Native proof helper exited (${code ?? signal}): ${stderr}`));
      resolveExit();
    });
  });
  child.stderr.on('data', bytes => {stderr = (stderr + String(bytes)).slice(-64 * 1024);});
  child.stdout.on('data', bytes => {
    try {
      buffer += String(bytes);
      assert(buffer.length <= 1024 * 1024, 'Oversized native proof response');
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const response = JSON.parse(buffer.slice(0,newline)); buffer = buffer.slice(newline+1);
        assert(pending && response.id === pending.id && typeof response.verified === 'boolean', 'Unexpected native proof response');
        if (response.verified) {
          assert(Array.isArray(response.proof) && response.proof.length > 0 && response.proof.length <= 4096 &&
            response.proof.every(value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)) &&
            new Set(response.proof).size === response.proof.length, 'Invalid reconstructed proof references');
        } else assert(response.needFullProof === true, 'Invalid native proof retry');
        const request = pending; pending = null; clearTimeout(request.timer); request.resolve(response);
      }
    } catch (error) {fail(error); child.kill();}
  });
  child.stdin.on('error', fail);
  return {
    verify(request) {
      if (failure) return Promise.reject(failure);
      assert(!pending && !closed, 'Native proof helper unavailable or already busy');
      const id = nextId++, encoded = JSON.stringify({...request,id}) + '\n';
      assert(Buffer.byteLength(encoded) <= 1024 * 1024, 'Oversized native proof request');
      return new Promise((resolveReply,reject) => {
        const timer = setTimeout(() => {fail(new Error('Native proof check timed out')); child.kill();}, 60_000);
        pending = {id,resolve:resolveReply,reject,timer}; child.stdin.write(encoded);
      });
    },
    async close() {
      child.stdin.end();
      const timer = setTimeout(() => child.kill(), 5000);
      try {await exited;} finally {clearTimeout(timer);}
      if (failure) throw failure;
    },
  };
}
export async function captureBurnHistory({output, first = FIRST_BURN_BLOCK, endpoint = DEFAULT_ENDPOINT,
  maxPages = 1, rpc: suppliedRpc, verifier, verifierSha256, proofVerifier: suppliedProofVerifier, onProgress = () => {}}) {
  // A page limit is mandatory: the full V0 code-proof stream is very large.
  // Operators select a reviewed local archive lane for a complete capture.
  assert(Number.isSafeInteger(first) && first >= FIRST_BURN_BLOCK && first <= CHECKPOINT.number, 'Invalid historical burn range');
  assert(Number.isSafeInteger(maxPages) && maxPages >= 1 && maxPages <= 4096, 'max-pages must be 1..4096');
  assert(Boolean(verifier) === Boolean(verifierSha256), 'Verifier path and SHA-256 are required together');
  assert(!suppliedProofVerifier || !verifier, 'Use only one proof verifier');
  output = resolve(output); await mkdir(output, {recursive: true});
  const rpc = suppliedRpc ?? new ReadOnlyRpc(endpoint);
  const configuration = {schema: 'sora-nexus-burn-capture-config-v1', first, last: CHECKPOINT.number,
    checkpointHash: CHECKPOINT.hash, pageSize: PAGE_SIZE, endpoint: rpc.endpoint ?? endpoint};
  await immutable(join(output, 'capture-config.json'), json(configuration));
  let proofVerifier = suppliedProofVerifier, proofInitialized = false;
  try {
    assert(await rpc.call('chain_getBlockHash', [CHECKPOINT.number]) === CHECKPOINT.hash, 'Archive is on another branch');
    const initialPath = join(output, 'initial.json');
    let initial;
    try {initial = JSON.parse(await readFile(initialPath, 'utf8'));}
    catch (error) {
      if (error.code !== 'ENOENT') throw error;
      const hash = await rpc.call('chain_getBlockHash', [first - 1]);
      assert(isHash(hash), 'Missing initial parent hash');
      const [header, proof] = await Promise.all([rpc.call('chain_getHeader', [hash]), rpc.call('state_getReadProof', [[CODE_KEY], hash])]);
      assert(Number(BigInt(header.number)) === first - 1, 'Wrong initial parent height');
      initial = {hash, header, proof: await proofObjects(output, proof, hash)};
      await immutable(initialPath, json(initial));
    }
    assert(Number(BigInt(initial.header.number)) === first - 1 && isHash(initial.hash), 'Cached initial parent mismatch');
    const pages = []; let next = first, previousHash = initial.hash, downloaded = 0, previousEvidence = initial;
    while (next <= CHECKPOINT.number) {
      const file = `pages/${next}.json.gz`, path = join(output, file);
      let encoded, page, downloadedPage = false;
      try {
        assert((await stat(path)).size <= MAX_PAGE_BYTES, 'Oversized cached burn page');
        encoded = await readFile(path); page = JSON.parse(gunzipSync(encoded, {maxOutputLength: MAX_PAGE_BYTES}));
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
        if (downloaded === maxPages) break;
        if (!proofVerifier && verifier) proofVerifier = await nativeProofSession(verifier, verifierSha256, output);
        if (proofVerifier && !proofInitialized) {
          const initialCheck = await proofVerifier.verify({initial:true,...previousEvidence});
          assert(initialCheck.verified, 'Cached parent code proof is incomplete');
          proofInitialized = true;
        }
        const records = [];
        // Sequential block reads limit archive load and large code-proof responses.
        for (let number = next; number < Math.min(next + PAGE_SIZE, CHECKPOINT.number + 1); ++number) {
          const hash = await rpc.call('chain_getBlockHash', [number]);
          const [block, proof] = await Promise.all([rpc.call('chain_getBlock', [hash]),
            rpc.call('state_getReadProof', [proofVerifier ? [EVENTS_KEY, CODE_PREFIX_KEY] : [EVENTS_KEY, CODE_KEY], hash])]);
          validateBlock(block, number, hash, records.at(-1)?.hash ?? previousHash);
          let references = await proofObjects(output, proof, hash);
          if (proofVerifier) {
            const request = {initial:false,hash,header:block.block.header,proof:references};
            let checked = await proofVerifier.verify(request);
            if (!checked.verified) {
              assert(checked.needFullProof === true, 'Unexpected incremental proof failure');
              references = await proofObjects(output, await rpc.call('state_getReadProof', [[EVENTS_KEY, CODE_KEY], hash]), hash);
              checked = await proofVerifier.verify({...request,proof:references});
              assert(checked.verified, 'Full historical proof remains incomplete');
            }
            references = checked.proof;
          }
          records.push({hash, block, proof: references});
        }
        page = {schema: 'sora-nexus-burn-proof-page-v1', first: next, records};
        const raw = json(page); assert(raw.length <= MAX_PAGE_BYTES, 'Burn page exceeds capture bound');
        encoded = gzipSync(raw); await immutable(path, encoded); downloaded += 1; downloadedPage = true;
      }
      assert(page.schema === 'sora-nexus-burn-proof-page-v1' && page.first === next &&
        page.records.length === Math.min(PAGE_SIZE, CHECKPOINT.number - next + 1), 'Cached burn page range mismatch');
      for (const record of page.records) {
        validateBlock(record.block, next, record.hash, previousHash);
        // A repaired hole can be followed by an existing page. Keep the live
        // helper's authenticated ancestry in step without fetching it again.
        // The ordinary contiguous cached prefix still needs no helper work.
        if (proofVerifier && proofInitialized && !downloadedPage) {
          const checked = await proofVerifier.verify({initial:false,hash:record.hash,
            header:record.block.block.header,proof:record.proof});
          assert(checked.verified, 'Cached historical proof remains incomplete');
        }
        previousHash = record.hash; next += 1;
        previousEvidence = {hash:record.hash,header:record.block.block.header,proof:record.proof};
      }
      pages.push({file, sha256: sha256(encoded)});
      onProgress({through: next - 1, downloadedPages: downloaded, totalPages: pages.length, complete: next > CHECKPOINT.number});
    }
    if (next <= CHECKPOINT.number) return {complete: false, first, through: next - 1, downloadedPages: downloaded,
      independentProofVerification: false, coverageComplete: false};
    assert(previousHash === CHECKPOINT.hash, 'Historical stream does not end at the checkpoint');
    const initialBytes = await readFile(initialPath);
    const manifest = {schema: 'sora-nexus-burn-proof-stream-v1', first, last: CHECKPOINT.number,
      checkpointHash: CHECKPOINT.hash, initial: {file: 'initial.json', sha256: sha256(initialBytes)}, pages};
    await immutable(join(output, 'burn-history-manifest.json'), json(manifest));
    return {complete: true, first, through: next - 1, downloadedPages: downloaded,
      independentProofVerification: false, coverageComplete: first === FIRST_BURN_BLOCK};
  } finally {
    if (!suppliedRpc) rpc.close();
    if (proofVerifier && !suppliedProofVerifier) await proofVerifier.close();
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const options = {}, args = process.argv.slice(2);
  for (let i = 0; i < args.length; i += 2) {
    assert(['--output', '--first', '--endpoint', '--max-pages', '--verifier', '--verifier-sha256'].includes(args[i]) && args[i + 1] && !Object.hasOwn(options, args[i]), 'Invalid capture arguments');
    options[args[i]] = args[i + 1];
  }
  assert(options['--output'], '--output DIR is required');
  console.log(JSON.stringify(await captureBurnHistory({output: options['--output'], endpoint: options['--endpoint'],
    verifier: options['--verifier'], verifierSha256: options['--verifier-sha256'],
    first: options['--first'] ? Number(options['--first']) : undefined, maxPages: options['--max-pages'] ? Number(options['--max-pages']) : 1,
    onProgress: progress => console.log(JSON.stringify(progress))})));
}
