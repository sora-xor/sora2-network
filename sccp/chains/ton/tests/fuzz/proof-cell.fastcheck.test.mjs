import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { Cell } from '@ton/core';
import fc from 'fast-check';

const encoderScript = join(process.cwd(), 'scripts', 'encode_sora_proof_cell.mjs');
const DEFAULT_RUNS = 300;

function resolveRuns(defaultRuns) {
  for (let i = 0; i < process.argv.length; i += 1) {
    if (process.argv[i] === '--fuzz-runs') {
      const raw = process.argv[i + 1];
      const parsed = Number(raw);
      if (!Number.isInteger(parsed) || parsed <= 0) {
        throw new Error(`--fuzz-runs must be a positive integer (got: ${raw})`);
      }
      return parsed;
    }
  }
  return defaultRuns;
}
const FUZZ_RUNS = resolveRuns(DEFAULT_RUNS);

function runEncoder(inputPath) {
  return spawnSync(process.execPath, [encoderScript, '--input', inputPath], {
    encoding: 'utf8',
  });
}

function runEncoderToOutput(inputPath, outputPath) {
  return spawnSync(process.execPath, [encoderScript, '--input', inputPath, '--output', outputPath], {
    encoding: 'utf8',
  });
}

function writeJson(dir, name, value) {
  const path = join(dir, name);
  writeFileSync(path, JSON.stringify(value), 'utf8');
  return path;
}

function writeText(dir, name, value) {
  const path = join(dir, name);
  writeFileSync(path, value, 'utf8');
  return path;
}

function baseProof() {
  return {
    mmr_proof: {
      leaf_index: 1,
      leaf_count: 2,
      items: ['0x' + '11'.repeat(32), '0x' + '22'.repeat(32)],
    },
    mmr_leaf: {
      version: 1,
      parent_number: 42,
      parent_hash: '0x' + '33'.repeat(32),
      next_authority_set_id: 7,
      next_authority_set_len: 2,
      next_authority_set_root: '0x' + '44'.repeat(32),
      random_seed: '0x' + '55'.repeat(32),
    },
    digest_scale: '0x01020304',
  };
}

test('fuzz: proof-cell encoder rejects malformed payload shapes', async () => {
  const runs = FUZZ_RUNS;
  await fc.assert(
    fc.property(fc.uint8Array({ minLength: 0, maxLength: 256 }), (bytes) => {
      const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
      try {
        const invalid = { ...baseProof(), digest_scale: `0x${Buffer.from(bytes).toString('hex')}` };
        delete invalid.mmr_leaf.parent_hash;
        const inputPath = writeJson(dir, 'invalid.json', invalid);
        const res = runEncoder(inputPath);
        assert.notEqual(res.status, 0);
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    }),
    { numRuns: runs },
  );
});

test('fuzz: proof-cell encoder rejects random JSON blobs fail-closed', async () => {
  const runs = FUZZ_RUNS;
  await fc.assert(
    fc.property(fc.string(), (text) => {
      const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
      try {
        const path = join(dir, 'random.json');
        writeFileSync(path, text, 'utf8');
        const res = runEncoder(path);
        assert.notEqual(res.status, 0);
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    }),
    { numRuns: runs },
  );
});

test('proof-cell encoder accepts a canonical minimal valid input', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
  try {
    const inputPath = writeJson(dir, 'valid.json', baseProof());
    const res = runEncoder(inputPath);
    assert.equal(res.status, 0);
    assert.match(res.stdout, /boc_hex=0x/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('proof-cell encoder rejects digest_scale values above the verifier single-cell limit', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
  try {
    const inputPath = writeJson(dir, 'oversized-digest.json', {
      ...baseProof(),
      digest_scale: `0x${'aa'.repeat(128)}`,
    });
    const res = runEncoder(inputPath);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /digest_scale exceeds the verifier single-cell limit of 127 bytes/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('proof-cell encoder preserves full uint64 precision for proof and leaf fields', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
  try {
    const largeU64 = '9007199254740993';
    const inputPath = writeJson(dir, 'large-u64.json', {
      ...baseProof(),
      mmr_proof: {
        leaf_index: largeU64,
        leaf_count: 1,
        items: [],
      },
      mmr_leaf: {
        ...baseProof().mmr_leaf,
        next_authority_set_id: largeU64,
        next_authority_set_len: 1,
      },
    });
    const outputPath = join(dir, 'proof.boc');
    const res = runEncoderToOutput(inputPath, outputPath);
    assert.equal(res.status, 0);

    const cell = Cell.fromBoc(readFileSync(outputPath))[0];
    const s = cell.beginParse();
    const leafIndex = s.loadUintBig(64);
    s.loadUintBig(64);
    s.loadRef();
    const leafRef = s.loadRef();
    const ls = leafRef.beginParse();
    ls.loadUint(8);
    ls.loadUint(32);
    ls.loadUintBig(256);
    const nextAuthoritySetId = ls.loadUintBig(64);

    assert.equal(leafIndex, 9007199254740993n);
    assert.equal(nextAuthoritySetId, 9007199254740993n);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('proof-cell encoder rejects malformed digest_scale hex instead of truncating it', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
  try {
    const inputPath = writeJson(dir, 'bad-digest-hex.json', {
      ...baseProof(),
      digest_scale: '0x01020g',
    });
    const res = runEncoder(inputPath);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /digest_scale must contain only hex digits/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('proof-cell encoder rejects malformed fixed-width hash hex instead of padding it', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
  try {
    const inputPath = writeJson(dir, 'bad-parent-hash.json', {
      ...baseProof(),
      mmr_leaf: {
        ...baseProof().mmr_leaf,
        parent_hash: `0x${'33'.repeat(31)}3`,
      },
    });
    const res = runEncoder(inputPath);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /mmr_leaf.parent_hash must have an even number of hex digits/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('proof-cell encoder rejects unsafe JSON number tokens for uint64 fields', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-fuzz-'));
  try {
    const inputPath = writeText(
      dir,
      'unsafe-u64.json',
      `{
  "mmr_proof": {
    "leaf_index": 9007199254740993,
    "leaf_count": 1,
    "items": []
  },
  "mmr_leaf": {
    "version": 1,
    "parent_number": 42,
    "parent_hash": "0x${'33'.repeat(32)}",
    "next_authority_set_id": 7,
    "next_authority_set_len": 1,
    "next_authority_set_root": "0x${'44'.repeat(32)}",
    "random_seed": "0x${'55'.repeat(32)}"
  },
  "digest_scale": "0x01020304"
}`,
    );
    const res = runEncoder(inputPath);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /mmr_proof.leaf_index must be encoded as a string once above Number\.MAX_SAFE_INTEGER/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
