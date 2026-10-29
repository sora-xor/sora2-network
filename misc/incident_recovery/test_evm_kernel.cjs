'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const k=require('./evm/kernel.cjs'),b=(n,v=0)=>Buffer.alloc(n,v),h=k.hx,u=k.integer,enc=k.encode;
test('tracked kernel verification body is byte-identical to the reviewed candidate',()=>{
  const source=fs.readFileSync(path.join(__dirname,'evm/kernel.cjs'),'utf8'),provenance=JSON.parse(fs.readFileSync(path.join(__dirname,'evm/source-provenance.json')));
  assert.equal(provenance.originKernelSha256,'3428bb415fec2add20d8d5f4b8b8ab882aa8c87ec0fbf6b44019e210c6140ef5');
  const body=source.slice(source.indexOf(provenance.unchangedBodyStartsWith));
  assert.equal(crypto.createHash('sha256').update(body).digest('hex'),'d2fe7fc6d1b9e139a79f1c457f651748bd797c0a808a4ac5577391a397b761c0');
  assert.equal(provenance.unchangedBodySha256,'d2fe7fc6d1b9e139a79f1c457f651748bd797c0a808a4ac5577391a397b761c0');
  const policy=JSON.parse(fs.readFileSync(path.join(__dirname,'evm/dependencies.json')));
  assert.equal(Object.hasOwn(policy,'root'),false);assert.equal(path.isAbsolute(policy.resolutionPackage),false);assert.equal(policy.files.length,5);
  const lock=JSON.parse(fs.readFileSync(path.resolve(__dirname,'evm',policy.lockfile)));
  assert.equal(lock.packages['node_modules/@noble/hashes'].version,policy.version);
});
test('primary Ethereum trie roots retain exact parity across input order and empty roots',()=>{
  const cases=[
    [[['A','a'.repeat(50)]],'d23786fb4a010da3ce639d66d5e904a11dbc02746d1ce25029e53290cabf28ab'],
    [[['doe','reindeer'],['dog','puppy'],['dogglesworth','cat']],'8aad789dff2f538bca5d8ea56e8abe10f4c7ba3a5dea95fea4cd6e7c3a1168d3'],
    [[['do','verb'],['horse','stallion'],['doge','coin'],['dog','puppy']],'5991bb8c6514148a29db676a14ac506cd2cd5775ace63c30a4fe457715e9ac84'],
    [[['foo','bar'],['food','bass']],'17beaa1648bafa633cda809c90c04af50fc8aed3cb40d16efbddee6fdf63c4c3'],
    [[['be','e'],['dog','puppy'],['bed','d']],'3f67c7a47520f79faa29255d2d3c084a7a6df0453116ed7232ff10277a8be68b'],
    [[['test','test'],['te','testy']],'8452568af70d8d140f58d941338542f645fcca50094b20f3c3d8c3df49337928'],
  ];
  for(const [pairs,want]of cases){const rows=pairs.map(row=>row.map(v=>Buffer.from(v)));assert.equal(h(k.trieRoot(rows)),'0x'+want);assert.equal(h(k.trieRoot(rows.reverse())),'0x'+want);}
  assert.equal(h(k.indexedRoot([])),'0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421');
  assert.throws(()=>k.trieRoot([[b(1),b(1,1)],[b(1),b(1,2)]]),/duplicate/);
});
test('published Keccak/RLP primitives and strict noncanonical bounds stay intact',()=>{
  assert.equal(h(k.keccak(b(0))),'0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470');
  assert.equal(h(enc(Buffer.from('dog'))),'0x83646f67');assert.equal(h(enc([Buffer.from('cat'),Buffer.from('dog')])),'0xc88363617483646f67');
  assert.equal(h(enc(u(0))),'0x80');assert.equal(h(enc(u(127))),'0x7f');assert.equal(h(enc(u(128))),'0x8180');
  for(const value of ['8100','b80100','b9003800','f80180','c18000','c281','8200','8000','ff'])assert.throws(()=>k.decode(Buffer.from(value,'hex')),/RLP/);
  let value=b(0);for(let i=0;i<18;i++)value=[value];assert.throws(()=>k.decode(enc(value)),/depth/);
});
test('header shapes are bounded and consistently hashed without a consensus claim',()=>{
  const fields=[b(32),k.keccak(enc([])),b(20),b(32,7),k.indexedRoot([]),k.indexedRoot([]),b(256),u(1),u(10),u(1000000),u(0),u(1000),b(0),b(32),b(8)];
  for(const count of [15,16,17,20,21]){
    const all=[...fields,u(1),b(32),u(0),u(0),b(32),b(32)].slice(0,count),raw=enc(all),result=k.header(raw);
    assert.equal(h(result.hash),h(k.keccak(raw)));assert.equal(result.number,10n);
  }
  assert.throws(()=>k.header(enc([...fields,u(1),b(32),u(0)])),/layout/);
  const invalid=fields.slice();invalid[10]=u(1000001);assert.throws(()=>k.header(enc(invalid)),/gasUsed/);
});
