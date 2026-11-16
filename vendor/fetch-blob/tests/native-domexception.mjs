import assert from 'node:assert/strict'
import { mkdtemp, writeFile, utimes, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import test from 'node:test'

const entry = process.argv[2]
  ? pathToFileURL(resolve(process.argv[2], 'from.js'))
  : new URL('../from.js', import.meta.url)
const { blobFrom, blobFromSync, fileFrom, fileFromSync } = await import(entry)

async function fixture (t) {
  const directory = await mkdtemp(join(tmpdir(), 'sora-fetch-blob-'))
  t.after(() => rm(directory, { recursive: true, force: true }))
  const file = join(directory, 'fixture.txt')
  await writeFile(file, 'SORA Nexus 日本語')
  await utimes(file, new Date(1000000), new Date(1000000))
  return file
}

test('sync and async file-backed Blobs and Files preserve content and metadata', async t => {
  const file = await fixture(t)
  for (const create of [blobFromSync, blobFrom, fileFromSync, fileFrom]) {
    const blob = await create(file, 'text/plain')
    assert.equal(await blob.text(), 'SORA Nexus 日本語')
    assert.equal(blob.type, 'text/plain')
    assert.equal(blob.size, Buffer.byteLength('SORA Nexus 日本語'))
    if ('name' in blob) {
      assert.equal(blob.name, 'fixture.txt')
      assert.equal(blob.lastModified, 1000000)
    }
  }
})

test('slicing and streaming preserve byte ranges', async t => {
  const blob = await blobFrom(await fixture(t))
  const chunks = []
  for await (const chunk of blob.slice(5, 10).stream()) chunks.push(chunk)
  assert.equal(Buffer.concat(chunks).toString(), 'Nexus')
})

test('changed-file reads reject with the native DOMException constructor', async t => {
  const file = await fixture(t)
  const blob = await blobFrom(file)
  await writeFile(file, 'changed')
  await utimes(file, new Date(2000000), new Date(2000000))
  await assert.rejects(blob.arrayBuffer(), error =>
    error instanceof globalThis.DOMException && error.name === 'NotReadableError')
})
