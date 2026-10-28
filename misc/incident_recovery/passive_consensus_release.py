"""Fixed fresh passive-consensus release hook. No archived success admission."""
from pathlib import Path
import hashlib
import json

import passive_consensus as passive


def source_binding():
    files = [Path(__file__).resolve(), Path(passive.__file__).resolve(),
             Path(passive.identity.__file__).resolve(), Path(passive.build.__file__).resolve()]
    return hashlib.sha256(json.dumps({p.name:passive.digest(p) for p in files},
                                     sort_keys=True,separators=(',',':')).encode()).hexdigest()


def validate_release(report, paths, manifest):
    value = passive.validate_manifest(passive.identity.strict_json(manifest.read_bytes()))
    for field, role in [('node','nodeBinary'),('chainSpec','chainSpec'),('bootstrapWasm','bootstrapWasm')]:
        passive.need(role in paths, 'release missing ' + role)
        source, pin = passive.file_input(manifest.parent,value[field])
        passive.need(pin == passive.digest(paths[role]), 'passive artifact differs from release ' + role)
    config_path, _ = passive.file_input(manifest.parent,value['recoveryConfig'])
    config = passive.identity.strict_json(config_path.read_bytes())
    passive.need(report.get('policy') and config.get('policyCommitment') == '0x'+report['policy']['commitment'],
                 'passive original policy differs from release')
    passive.need('finalWasm' in paths and value['target']['codeHash'] == config['finalCodeHash']
                 == '0x'+passive.digest(paths['finalWasm'])['blake2_256'] and value['target']['phase']=='Complete',
                 'passive terminal target must contain the installed exact final release Wasm')
    return value


def package_hook(report, paths, manifest, expected_source_sha256, output):
    passive.need(source_binding() == expected_source_sha256, 'fixed passive hook/source differs')
    manifest = Path(manifest).absolute()
    passive.identity.physical(manifest)
    value = validate_release(report,paths,manifest)
    original = {role:passive.digest(path) for role,path in paths.items()}
    harness = passive.Harness(manifest,output)
    result = harness.run()
    passive.need(source_binding() == expected_source_sha256, 'fixed passive verifier changed during run')
    passive.need(original == {role:passive.digest(path) for role,path in paths.items()}, 'release input changed during passive verification')
    passive.need(result['node'] == original['nodeBinary'] and result['chainSpec'] == original['chainSpec'], 'fresh result identity differs')
    passive.check_report(result['fullAndWarpSynchronization']['full'],value['target'],result['policyCommitment'],full=True)
    warped = result['fullAndWarpSynchronization']['warp']
    observed = warped.get('observedWarp')
    passive.need(isinstance(observed,dict) and type(observed.get('pid')) is int, 'actual warp process absent')
    passive.check_report(warped,value['target'],result['policyCommitment'],observed['pid'])
    additions=[]
    added_paths={}
    total=0
    # Only explicitly retained public files are eligible. Never scan node base
    # directories: their generated libp2p identities are private test material.
    for index,(name,pin) in enumerate(sorted(harness.pins.items())):
        path=Path(name)
        if not path.is_relative_to(harness.output): continue
        total += pin['bytes']
        passive.need(total <= 16 * 1024**3, 'passive public evidence byte bound')
        role=f'passiveConsensusEvidence{index:06}'
        passive.need(role not in paths and passive.digest(path)==pin, 'passive output role collision/change')
        additions.append({'role':role,'sourcePath':'generated-passive-consensus/'+path.relative_to(harness.output).as_posix(),'object':'objects/'+pin['sha256'],**pin,'executable':False})
        added_paths[role]=path
    passive.need(len(additions)<=32768 and len(additions)+len(report['artifacts'])<=65536,'passive evidence inventory bound')
    gate={'status':'verified',
        'reason':'Fresh passive full/warp nodes crossed the pinned forced boundary; stopped independent databases and actual accepted warp proof bytes were reverified by the exact node',
        'sourceBinding':expected_source_sha256,'completionSha256':passive.digest(harness.output/'completion.json')['sha256']}
    crash_coverage={'targets':[row['target']['hash'] for row in result['committedBoundaryCrashes']],
        'allRecoveryBlocksKilledAfterCommit':result['allRecoveryBlocksKilledAfterCommit'],
        'timing':'SIGKILL after committed/finalized block observation', 'inCommitCrashVerified':False}
    harness.originals()
    passive.need(source_binding() == expected_source_sha256, 'fixed passive verifier changed before registration')
    passive.need(original == {role:passive.digest(path) for role,path in paths.items()}, 'release input changed before registration')
    report['artifacts'].extend(additions)
    paths.update(added_paths)
    report['gates']['fullAndWarpSynchronization']=gate
    report['passiveCommittedBoundaryCrashCoverage']=crash_coverage
    # This limited host corridor cannot clear the broader chunk/escape/repair
    # failure-injection gate. Existing independently established gates stay intact.
    report['productionReady']=False
    return result
