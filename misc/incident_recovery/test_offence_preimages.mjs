import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
import {createRequire} from 'node:module';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {authenticateOriginal} from './prepare_offence_preimages.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const source=root+'/artifacts/recovery-28191817/offences-preimages-victim-v3/capture.json';
const available=existsSync(source);
function fixture(){
  const req=createRequire(root+'/runtime-rehearsal-async-staking-2026-10-25/validation/package.json');
  const {TypeRegistry,Metadata}=req('@polkadot/types');
  const registry=new TypeRegistry();registry.register(req('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,readFileSync(root+'/artifacts/recovery-28191817/checkpoint/runtime-metadata.scale'));registry.setMetadata(metadata);
  const reportType=metadata.asLatest.toJSON().pallets.find(p=>p.name==='Offences').storage.items.find(i=>i.name==='Reports').type.map.value;
  const offenderType=registry.lookup.getSiType(reportType).def.asComposite.fields[0].type.toNumber();
  const capture=JSON.parse(readFileSync(source)),rows=new Map(capture.states[0].values[0].changes);
  const affected=JSON.parse(readFileSync(root+'/artifacts/recovery-28191817/offences-review-v1/hash-inspection-victim-v3.json')).affected;
  return affected.map(r=>({registry,reportType,offenderType,kind:r.kind,slot:r.slot,reportId:r.id,originalValue:rows.get(r.reportKey),currentValue:r.reportValue,denominator:10n**38n}));
}
test('both actual checkpoint offender hashes authenticate and denominate exactly',{skip:!available},()=>{
  const inputs=fixture();assert.equal(inputs.length,2);
  for(const input of inputs)assert.match(authenticateOriginal(input),/^0x[0-9a-f]+$/);
});
test('wrong original identity, time slot, factor and trailing codec bytes are rejected',{skip:!available},()=>{
  const [input]=fixture();
  assert.throws(()=>authenticateOriginal({...input,originalValue:'0xff'+input.originalValue.slice(4)}),/ReportId/);
  assert.throws(()=>authenticateOriginal({...input,slot:'0x00000000'}),/ReportId/);
  assert.throws(()=>authenticateOriginal({...input,denominator:2n}),/denomination/);
  assert.throws(()=>authenticateOriginal({...input,originalValue:input.originalValue+'00'}),/codec/);
});
test('denomination must preserve exact ordered nominator identities',{skip:!available},()=>{
  const [input]=fixture(),value=input.registry.createTypeUnsafe('Lookup'+input.reportType,[input.currentValue]);
  const json=value.toJSON();const others=json.offender[1].others;assert.ok(others.length>1);[others[0],others[1]]=[others[1],others[0]];
  const changed=input.registry.createTypeUnsafe('Lookup'+input.reportType,[json]).toHex();
  assert.throws(()=>authenticateOriginal({...input,currentValue:changed}),/account order/);
});
