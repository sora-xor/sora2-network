#!/usr/bin/env python3
"""Create-only preserved-engine recorder source; no Cargo, runtime or services.

This is source binding, not compiler provenance or production admission. Input
build/provenance records must already belong to the outer verified release.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import stat

HERE = Path(__file__).resolve().parent
MAX_JSON = 64 * 1024 * 1024
LEGACY_TREE = '36c466398e3f2760e8ab44fdb89de796f5b0dd99'
PACKAGE = 'utils/native-operation-recorder'

def require(condition, message):
    if not condition:
        raise ValueError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def read(path, maximum=MAX_JSON):
    require(path.is_absolute() and path.resolve() == path and path.is_file(), f'noncanonical file {path}')
    require(path.stat().st_size <= maximum, f'oversized input {path}')
    return path.read_bytes()

def json_bytes(data):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            require(key not in out, f'duplicate JSON key: {key}')
            out[key] = value
        return out
    return json.loads(data, object_pairs_hook=unique)

def relative_file(root, relative):
    rel = Path(relative)
    require(isinstance(relative, str) and '\\' not in relative and rel.as_posix() == relative
            and not rel.is_absolute() and rel.parts and all(x not in ('.', '..') for x in rel.parts), 'unsafe inventory path')
    path = root / rel
    require(path.resolve() == path and path.is_file(), f'missing/symlink source {path}')
    return path

def pin_read(path, expected):
    data = read(path)
    require(sha(data) == expected, f'checksum mismatch {path}')
    return data

def rust_array(values):
    return '[' + ','.join(str(x) for x in values) + ']'

def generated_bytes(source, name):
    matches = re.findall(r'pub const '+name+r': (?:&\[u8\]|\[u8; 32\]) = &?\[([0-9,\s]+)\];', source)
    require(len(matches) == 1, f'noncanonical generated {name}')
    return bytes(int(x) for x in matches[0].split(',') if x.strip())

def validate(source, build_path, build_sha, wasm_path, wasm_sha, policy_path, policy_sha):
    require(source.is_absolute() and source.resolve() == source and source.is_dir(), 'canonical source required')
    build_bytes = pin_read(build_path, build_sha)
    build = json_bytes(build_bytes)
    require(build.get('status') == 'passed' and type(build.get('exitCode')) is int
            and build.get('exitCode') == 0, 'actual completed build required')
    require(build.get('specVersion') == 134 and build.get('transactionVersion') == 131, 'only preserved134/131')
    require(Path(build['source']) == source, 'actual build source differs')
    require('--locked' in build['command'] and '--offline' in build['command'], 'bound offline locked build required')
    inventory_bytes = pin_read(build_path.parent/'source-files.json', build['sourceFingerprint']['sha256'])
    provenance_bytes = pin_read(build_path.parent/'source-provenance.json', build['sourceProvenance']['sha256'])
    provenance = json_bytes(provenance_bytes)
    require(provenance['deployedSourceTree'] == LEGACY_TREE and provenance['sdk'] == 'polkadot-stable2603-3', 'wrong native engine ancestry/SDK')
    require(provenance.get('bound') is True and provenance['specVersion'] == 134 and provenance['transactionVersion'] == 131, 'unbound or non-bootstrap export')
    require(read(source/'RECOVERY-PROVENANCE.json') == provenance_bytes, 'source provenance differs from executed Wasm build')
    inventory = json_bytes(inventory_bytes)
    require(isinstance(inventory, dict) and 1 <= len(inventory) <= 20000, 'source inventory bound')
    for name, pin in inventory.items():
        data = read(relative_file(source, name))
        require(type(pin['bytes']) is int and sha(data) == pin['sha256']
                and len(data) == pin['bytes'], f'build source drift {name}')
    require(build['lockBefore'] == build['lockAfter'] and inventory['Cargo.lock']['sha256'] == build['lockAfter']['sha256'], 'executed lock mismatch')
    code = pin_read(wasm_path, wasm_sha)
    require(wasm_path.parent == build_path.parent and build['artifacts'][wasm_path.name]['sha256'] == wasm_sha, 'Wasm not from actual build')
    policy = pin_read(policy_path, policy_sha)
    policy_commitment = hashlib.blake2b(policy, digest_size=32).digest()
    require(provenance['bindingEvidence']['policy']['sha256'] == policy_sha and provenance['bindingEvidence']['policy']['blake2_256'] == policy_commitment.hex(), 'policy build binding mismatch')
    generated = (source/'runtime/src/incident_recovery/generated.rs').read_text()
    for prefix in ('', 'ORIGINAL_'):
        require(generated_bytes(generated,prefix+'POLICY_SCALE') == policy and generated_bytes(generated,prefix+'POLICY_COMMITMENT') == policy_commitment, 'compiled policy differs; repair unsupported')
    require(type(build.get('fixtureOnly')) is bool and type(provenance.get('fixtureOnly')) is bool,
            'missing/invalid fixture marker')
    require(build['fixtureOnly'] == provenance['fixtureOnly'], 'contradictory fixture marker')
    fixture = build['fixtureOnly']
    require(build.get('productionReady') is False and provenance.get('releaseReady') is False, 'unexpected release assertion')
    return inventory, {
        'schema':'sora-nexus-native-operation-engine-binding-v1',
        'engineSha256':wasm_sha, 'engineBlake2':hashlib.blake2b(code,digest_size=32).hexdigest(),
        'policySha256':policy_sha, 'policyCommitment':policy_commitment.hex(),
        'sourceFingerprintSha256':sha(inventory_bytes), 'sourceProvenanceSha256':sha(provenance_bytes),
        'actualWasmBuildReceiptSha256':build_sha, 'deployedSourceTree':LEGACY_TREE,
        'sdk':'polkadot-stable2603-3', 'specVersion':134, 'transactionVersion':131,
        'fixtureOnly':fixture, 'productionReady':False,
        'nativeCompilerProvenanceVerified':False, 'nativeExecutionVerified':False,
    }

def prepare(args):
    template_bytes = {}
    for path in sorted((HERE/'template').rglob('*')):
        require(not path.is_symlink(), 'template symlink')
        if path.is_file():
            template_bytes[str(path.relative_to(HERE))] = read(path)
    require('template/runtime-native-wrapper.rs' in template_bytes, 'native wrapper absent')
    inventory, binding = validate(args.source, args.build_receipt, args.build_receipt_sha256,
                                  args.wasm, args.wasm_sha256, args.policy, args.policy_sha256)
    require(args.destination.is_absolute() and not args.destination.exists(), 'new absolute destination required')
    require(args.destination.parent.resolve() == args.destination.parent, 'destination parent symlink')
    args.destination.mkdir()
    for name, pin in inventory.items():
        target = args.destination/name
        target.parent.mkdir(parents=True,exist_ok=True)
        original = relative_file(args.source,name)
        with original.open('rb') as src, target.open('xb') as dst:
            shutil.copyfileobj(src,dst)
        target.chmod(stat.S_IMODE(original.stat().st_mode) | stat.S_IWUSR)
        require(sha(target.read_bytes()) == pin['sha256'], f'copy mismatch {name}')
    for source_name in ('source-files.json','source-provenance.json','build.json'):
        original = args.build_receipt if source_name == 'build.json' else args.build_receipt.parent/source_name
        with (args.destination/('RECORDER-ORIGINAL-'+source_name)).open('xb') as f:
            f.write(read(original))
    with (args.destination/'RECORDER-ORIGINAL-Cargo.lock').open('xb') as output:
        output.write(read(args.source/'Cargo.lock'))
    package = args.destination/PACKAGE
    require(not package.exists(), 'recorder package collision')
    for relative, data in template_bytes.items():
        if relative.startswith('template/recorder/'):
            target=package/relative.removeprefix('template/recorder/')
            target.parent.mkdir(parents=True,exist_ok=True)
            with target.open('xb') as out:
                out.write(data)
    wrapper=template_bytes['template/runtime-native-wrapper.rs']
    lib=args.destination/'runtime/src/lib.rs'
    require(b'native_recovery_recorder_' not in lib.read_bytes(), 'native wrapper already exists')
    with lib.open('ab') as output: output.write(b'\n'+wrapper)
    manifest=args.destination/'Cargo.toml'
    text=manifest.read_text()
    require(text.count('[workspace]\nmembers = [') == 1, 'unexpected preserved workspace declaration')
    manifest.write_text(text.replace('[workspace]\nmembers = [','[workspace]\nmembers = [\n    "'+PACKAGE+'",',1))
    binding['recorderTemplate']={name:sha(data) for name,data in template_bytes.items()}
    binding['preparerSha256']=sha(read(HERE/'prepare.py'))
    binding['originalLockSha256']=inventory['Cargo.lock']['sha256']
    encoded=json.dumps(binding,sort_keys=True,separators=(',',':'))
    require('"####' not in encoded, 'binding Rust literal collision')
    generated=(f'pub const ENGINE_SHA256: &str = "{binding["engineSha256"]}";\n'
               f'pub const ENGINE_BLAKE2: [u8;32] = {rust_array(bytes.fromhex(binding["engineBlake2"]))};\n'
               f'pub const POLICY_COMMITMENT: [u8;32] = {rust_array(bytes.fromhex(binding["policyCommitment"]))};\n'
               f'pub const FIXTURE_ONLY: bool = {str(binding["fixtureOnly"]).lower()};\n'
               f'pub const BINDING_JSON: &str = r####"{encoded}"####;\n')
    (package/'src/binding.rs').write_text(generated)
    (args.destination/'RECORDER-ENGINE-BINDING.json').write_text(json.dumps(binding,indent=2)+'\n')
    # Source drift during copying must fail; retain the failed destination rather than repair it.
    validate(args.source,args.build_receipt,args.build_receipt_sha256,args.wasm,args.wasm_sha256,args.policy,args.policy_sha256)
    require({str(path.relative_to(HERE)):read(path) for path in sorted((HERE/'template').rglob('*')) if path.is_file()}
            == template_bytes, 'recorder templates changed during preparation')
    changes={name:sha((args.destination/name).read_bytes()) for name in ('Cargo.toml','runtime/src/lib.rs')}
    recorder_inventory={str(path.relative_to(args.destination)):{'sha256':sha(path.read_bytes()),'bytes':path.stat().st_size}
                        for path in sorted(args.destination.rglob('*')) if path.is_file()}
    with (args.destination/'RECORDER-SOURCE-FILES.json').open('x') as output:
        json.dump(recorder_inventory,output,indent=2);output.write('\n')
    return {'destination':str(args.destination),'originalSourceFiles':len(inventory),'amendedHostOnlySources':changes,
            'engineBindingSha256':sha((args.destination/'RECORDER-ENGINE-BINDING.json').read_bytes()),
            'nativeBuildExecuted':False,'lockResolutionExecuted':False,'runtimeExecuted':False,'productionReady':False}

if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    for name in ('source','build-receipt','wasm','policy','destination'):
        parser.add_argument('--'+name,type=Path,required=True)
    for name in ('build-receipt','wasm','policy'):
        parser.add_argument('--'+name+'-sha256',required=True)
    print(json.dumps(prepare(parser.parse_args()),indent=2))
