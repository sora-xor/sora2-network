#!/usr/bin/env python3
"""Verify packaged runtime, proposal bytes and unsigned-call hashes offline."""
from pathlib import Path
import hashlib,json,sys
root=Path(__file__).resolve().parent.parent
wasm=(root/'framenode-runtime-4.8.9.compact.compressed.wasm').read_bytes()
preimage=json.loads((root/'preimage.json').read_text())
go=json.loads((root/'governance-calls.json').read_text())
info=json.loads((root/'runtime-upgrade-4.8.9-info.json').read_text())
blake=lambda b:'0x'+hashlib.blake2b(b,digest_size=32).hexdigest()
call=(root/'set-code-call.scale').read_bytes()
assert call[:2]==bytes([0,2])
assert len(wasm)<2**30
assert len(call)<=4*1024*1024
compact=((len(wasm)<<2)|2).to_bytes(4,'little')
assert call==bytes([0,2])+compact+wasm
assert preimage['wasm_blake2_256']==go['wasmBlake2']==blake(wasm)
assert preimage['proposal_hash']==go['proposalHash']==blake(call)
assert preimage['proposal_len']==go['proposalLength']==len(call)
assert info['wasm']['sha256']==hashlib.sha256(wasm).hexdigest()
assert info['wasm']['blake2_256']==blake(wasm)
assert info['wasm']['bytes']==len(wasm)
assert info['proposal']['hash']==blake(call)
assert info['proposal']['bytes']==len(call)
assert bytes.fromhex((root/'set-code-call.hex').read_text().strip()[2:])==call
for name,value in go['calls'].items():
 b=bytes.fromhex((root/value['file']).read_text().strip()[2:])
 assert len(b)==value['length'],name
 assert blake(b)==value['hash'],name
manifest=root/'SHA256SUMS'
assert manifest.is_file(),'SHA256SUMS is required'
listed=set()
for line in manifest.read_text().splitlines():
 digest,name=line.split('  ',1)
 assert name not in listed,('duplicate checksum',name)
 listed.add(name)
 assert hashlib.sha256((root/name).read_bytes()).hexdigest()==digest,name
actual={str(path.relative_to(root)) for path in root.rglob('*') if path.is_file() and path!=manifest}
assert listed==actual,{'missingChecksums':sorted(actual-listed),'missingFiles':sorted(listed-actual)}
print(json.dumps({'passed':True,'wasmBytes':len(wasm),'wasmSha256':hashlib.sha256(wasm).hexdigest(),'proposalHash':blake(call),'proposalBytes':len(call),'unsignedCallsVerified':len(go['calls']),'packageFilesVerified':len(listed)},indent=2))
