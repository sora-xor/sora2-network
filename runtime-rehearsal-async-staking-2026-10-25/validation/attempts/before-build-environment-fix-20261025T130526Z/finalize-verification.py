#!/usr/bin/env python3
"""Aggregate async staking migration evidence after independently rechecking its bindings.

Default execution refuses unfinished native reports and writes only the final summary.
--precheck validates completed non-native gates without writing anything.
No builds, network requests, frozen-evidence mutation, or Wasm execution are performed.
"""
import argparse
import datetime
import hashlib
import gzip
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
RUN = ROOT / 'runtime-rehearsal-async-staking-2026-10-25'
V = RUN / 'validation'
CANDIDATE = None
SOURCE = None
UPSTREAM = '7a8daa68e18718c18823c1109513fdc1c8aa71fe'
EXPECTED_BUILD = ['source-start', 'clippy-all', 'source-after-clippy', 'format', 'production-feature-graph', 'production-wasm', 'source-after-build']
EXPECTED_STAGES = [
    'Current finalized production runtime identity', 'Verify full source before rehearsal',
    'Wasm interface negative regressions', 'Read-only RPC guard regressions',
    'Fee evidence binding regressions', 'Run evidence negative regressions',
    'Runtime lifecycle and rollback regressions', 'SDK2609 compatibility exception regressions',
    'Authorized heartbeat fixture encoding regressions', 'SDK source provenance negative regressions',
    'SDK primitives source reconstruction', 'Async staking source provenance negative regressions',
    'Async staking source reconstruction', 'Exact candidate compatibility',
    'Independent complete bridge payload recovery',
    'Offline candidate governance signing compatibility', 'Equivocation and fee execution',
    'Independent fee event verification', 'Fee policy and bridge fixtures',
    'Bridge readiness evidence regressions', 'Complete pending bridge inventory',
    'Current finalized bridge readiness', 'Independent bridge readiness verification', 'Verify full source after rehearsal',
]
EXPECTED_NATIVE = [
    'source-start', 'runtime-production', 'runtime-local-remote-name',
    'staking-async-all', 'bridge-operator-all', 'bridge-multisig-all', 'native-pinned-production', 'source-end',
]
verified = {}
stamps = {}

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def path(value, base=V):
    result = Path(value)
    return result if result.is_absolute() else base / result

def sha(p):
    p = Path(p).resolve()
    require(p.is_file(), f'Missing evidence/input: {p}')
    stat = p.stat()
    stamp = (stat.st_ino, stat.st_size, stat.st_mtime_ns)
    if p in stamps:
        require(stamps[p] == stamp, f'File changed during aggregation: {p}')
        return verified[str(p)]
    h = hashlib.sha256()
    with p.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    after = p.stat()
    require(stamp == (after.st_ino, after.st_size, after.st_mtime_ns), f'File changed while hashing: {p}')
    stamps[p] = stamp
    verified[str(p)] = h.hexdigest()
    return h.hexdigest()

def bound(p, expected):
    actual = sha(p)
    require(isinstance(expected, str) and actual == expected, f'Hash mismatch: {p}: expected {expected}, actual {actual}')
    return actual

def read(name, passed=False, base=V):
    p = path(name, base)
    sha(p)
    d = json.loads(p.read_text())
    if passed:
        require(d.get('status') == 'passed', f'Gate not passed: {p}: {d.get("status")}')
    return d

def hash_map(values, base=ROOT):
    for name, digest in values.items():
        bound(path(name, base), digest)

def all_true(values, label, exceptions=()):
    require(values, f'Empty check set: {label}')
    for name, value in values.items():
        if name not in exceptions:
            require(value is True, f'{label}.{name} is not true: {value}')

def source_check(name, fingerprint):
    d = read(name, True)
    require(d['sourceSha256'] == SOURCE == fingerprint['source']['sha256'], f'{name}: different source')
    require(d['sourceFiles'] == fingerprint['source']['fileCount'] and d['gitHead'] == fingerprint['git']['head'], f'{name}: source inventory/HEAD mismatch')
    require(d['changes'] == [] and d['compared'] == 'source', f'{name}: source changes recorded')
    return d

def rust_counts(text):
    return [dict(zip(('passed', 'failed', 'ignored'), map(int, row))) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)]

def verify_build():
    global CANDIDATE, SOURCE
    build = read('final-build-verification.json', True)
    proof = read('candidate-build-evidence.json', True)
    fingerprint = read('source-clippy-start.json')
    CANDIDATE = proof['candidateSha256']
    SOURCE = proof['sourceFingerprint']['sourceSha256']
    require(re.fullmatch(r'[0-9a-f]{64}', CANDIDATE) and re.fullmatch(r'[0-9a-f]{64}', SOURCE), 'Invalid candidate/source hash')
    require(fingerprint['status'] == 'final', 'Source is not frozen/final')
    require(build['candidate'] == proof, 'Build report and candidate proof disagree')
    require([row['name'] for row in build['steps']] == EXPECTED_BUILD, 'Build gate set/order differs')
    for row in build['steps']:
        require(row['exitCode'] == 0 and row.get('finishedAt'), f'Incomplete build step: {row["name"]}')
        for field in ('stdout', 'stderr'):
            bound(V / row[field], row[field + 'Sha256'])
    require(proof['candidateBytes'] > 0, 'Empty candidate')
    require((proof['specVersion'], proof['transactionVersion']) == (137, 133), 'Unexpected runtime versions')
    bound(path(proof['candidatePath']), CANDIDATE)
    require(path(proof['candidatePath']).stat().st_size == proof['candidateBytes'], 'Candidate size differs')
    require(proof['wasmProjectDependency']['default-features'] is False and proof['wasmProjectDependency']['features'] == ['build-wasm-binary'], 'Nonproduction Wasm features')
    require(proof['sourceFingerprint'] == build['sourceFingerprint'], 'Build fingerprint proof differs')
    bound(V / proof['sourceFingerprint']['path'], proof['sourceFingerprint']['sha256'])
    require(proof['sourceFingerprint']['sourceSha256'] == SOURCE, 'Unexpected source')
    hash_map(proof['sourceInputs'])
    bound(V / 'source-fingerprint.py', fingerprint['recorderSha256'])
    for name in ['source-clippy-end-check.json', 'source-build-end-check.json', 'source-before-rehearsal.json', 'source-after-rehearsal.json']:
        source_check(name, fingerprint)
    lint = (V / 'final-build-logs/clippy-all.stderr.log').read_text()
    for mode in ('mainnet', 'try-runtime', 'extended'):
        require(f'Running Clippy ({mode})' in lint, f'Missing Clippy mode: {mode}')
    for line in (V / 'final-build-logs/clippy-all.stdout.log').read_text().splitlines():
        try:
            item = json.loads(line)
        except ValueError:
            continue
        if item.get('reason') == 'compiler-message':
            require(item['message']['level'] not in ('warning', 'error'), 'Clippy emitted a warning/error')
    return build, proof, fingerprint

def load_module(source, name):
    spec = importlib.util.spec_from_file_location(name, source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def verify_audits(run):
    runner_path = RUN / 'run-local-rehearsals.py'
    runner = load_module(runner_path, 'async_rehearsal_runner')
    files, supporting = runner.supporting_evidence(V, ROOT)
    require(supporting == run['supportingEvidence'], 'Current supporting evidence differs from rehearsal')
    for source in files: sha(source)
    sha(runner_path)
    cargo = read('current-cargo-audits/summary.json', True)
    npm = read('npm-security-refresh/summary.json', True)
    return {'cargoLockfiles': 11, 'cargoVulnerabilities': 0, 'cargoWarnings': 0,
            'regressionHarnessLockfiles': 5, 'npmGraphs': 8, 'npmVulnerabilities': 0,
            'rustSecDatabaseCommit': cargo['databaseCommit'],
            'cargoLockBindings': {x['lockfile']: x['lockSha256'] for x in cargo['audits']},
            'npmLockBindings': {str(Path(x['project']) / x['lockfile']): x['lock_sha256'] for x in npm['projects']},
            'cargoSummarySha256': sha(V / 'current-cargo-audits/summary.json'),
            'databaseSourceSha256': sha(ROOT / cargo['databaseSource']),
            'historicalSourceReviewSha256': supporting['historicalSourceReviewSha256'],
            'historicalHarnessScope': supporting['regressionHarnessExecution'],
            'npmSummarySha256': sha(V / 'npm-security-refresh/summary.json')}


def verify_wasm():
    run = read('rehearsal-run.json', True)
    require([s['name'] for s in run['stages']] == EXPECTED_STAGES, 'Missing/reordered Wasm stage')
    for s in run['stages']:
        require(s['status'] == 'passed' and s['exitCode'] == 0 and s.get('finishedAt'), f'Incomplete Wasm stage {s["name"]}')
        bound(RUN / s['log'], s['logSha256'])
        if 'result' in s:
            bound(RUN / s['result'], s['resultSha256'])
            read(RUN / s['result'], True)
    hash_map(run['inputs']['files'])
    all_true(run['checks'], 'Wasm run')
    require(run['inputs']['candidateSha256'] == CANDIDATE, 'Wasm candidate differs')
    bound(path(run['inputs']['candidatePath']), CANDIDATE)
    bound(path(run['inputs']['candidateSourcePath']), CANDIDATE)
    bound(V / 'snapshot.json', run['inputs']['snapshotSha256'])
    bound(V / 'candidate-build-evidence.json', run['inputs']['buildEvidenceSha256'])
    snapshot = read('snapshot.json', True)
    require(snapshot['blockHash'] == run['inputs']['snapshotBlockHash'], 'Pinned block differs')
    identity = read('current-production-identity.json', True)
    for key in ['sameRuntimeVersion', 'sameRuntimeCode', 'pinnedBlockRemainsCanonical', 'snapshotPreserved', 'pinnedRuntimeStillRepresentative']:
        require(identity[key] is True, f'Production identity mismatch: {key}')
    require(identity['readOnly'] and identity['submittedTransactions'] == 0, 'Production identity was not read-only')
    compatibility = read('wasm-compatibility.json', True)
    all_true(compatibility['checks'], 'Wasm compatibility', exceptions=('existingScaleEncodingCompatible',))
    require(compatibility['checks']['existingScaleEncodingCompatible'] is False, 'Raw event ABI break must remain visible')
    bound(V / 'snapshot.json', compatibility['snapshotSha256'])
    for item in compatibility['inputs'].values():
        bound(path(item['path']), item['sha256'])
        bound(path(item['metadataPath']), item['metadataSha256'])
        if 'metadataScalePath' in item:
            bound(path(item['metadataScalePath']), item['metadataScaleSha256'])
    metadata = read('metadata-compatibility.json')
    require(metadata['reviewedSdkMigrationCompatible'] is True and metadata['reviewedAsyncStakingMigrationCompatible'] is True and not metadata['compatibleExistingScaleEncoding'], 'Incorrect metadata compatibility classification')
    require(metadata['reviewedSignedExtensionChange']['requiresTransactionVersion'] == 133 and metadata['requiresStorageMigration'] == {'pallet': 'Staking', 'from': 16, 'to': 18, 'rawStateWitnessRequired': True}, 'Incorrect async migration contract')
    require([row['path'] for row in metadata['reviewedStorageMetadataWrappers']] == [
            'Staking.storage.BondedEras', 'Staking.storage.ClaimedRewards', 'Staking.storage.ErasRewardPoints.individual'],
            'Unexpected bounded storage metadata exception')
    require(not metadata['breakingChanges'] and not metadata['additions'], 'Unreviewed metadata changes')
    for item in metadata['inputs'].values():
        bound(path(item['path']), item['sha256'])
    python = read('python-candidate-signing-report.json', True)
    require(python['candidate_sha256'] == CANDIDATE and len(python['cases']) == 8, 'Python signing coverage differs')
    require(python['rpc_calls'] == python['submission_calls'] == 0, 'Signing check made network requests')
    hash_map(python['source_sha256'])
    for item in python['cases']:
        for key in ['full_extra_matches_all_metadata_extensions', 'signature_verifies', 'signature_rejects_transaction_version_131', 'signature_rejects_transaction_version_132', 'extrinsic_roundtrip_consumes_all_bytes']:
            require(item[key] is True, f'Incomplete signing check: {key}')
    suites = {}
    for name in ['equivocation-fee-wasm-rehearsal.json', 'fee-policy-wasm-rehearsal.json']:
        d = read(name, True)
        require(d['candidate']['sha256'] == CANDIDATE and d['inputs']['snapshotSha256'] == sha(V / 'snapshot.json'), f'Wrong suite input: {name}')
        require(d['pinnedBlockHash'] == snapshot['blockHash'] and d['submittedTransactions'] == d['privateKeysRead'] == 0, f'Wrong suite scope: {name}')
        all_true(d['checks'], name)
        require(d['initialize']['inclusionMode'] == 'OnlyInherents' and d['postUpgradeInitialize']['inclusionMode'] == 'AllExtrinsics', f'Incorrect block lifecycle: {name}')
        require(d['upgradeFinalization']['signedExtrinsicsApplied'] == 0 and d['upgradeFinalization']['cursorAfter'] is None, f'Incomplete migration lifecycle: {name}')
        suites[name] = d
    fee = read('fee-event-check.json', True)
    require(fee['candidateSha256'] == CANDIDATE and fee['allFeeEventsMatchPayerBalanceDecrease'] is True, 'Fee event binding failed')
    for key, name in [('snapshotSha256', 'snapshot.json'), ('metadataSha256', 'candidate-metadata.json'), ('compatibilitySha256', 'wasm-compatibility.json'), ('rehearsalSha256', 'equivocation-fee-wasm-rehearsal.json'), ('scriptSha256', 'verify-fee-events.py')]:
        bound(V / name, fee[key])
    return run, snapshot, metadata, suites

def verify_staking_migration(run, snapshot, policy):
    source = read('staking-async-source-proof.json', True)
    require(source['upstreamCommit'] == UPSTREAM and source['upstreamTag'] == 'polkadot-stable2609', 'Async staking upstream differs')
    require(source['verification'] == {'originalManifestMatchesGitObject': True,
            'manifestAdaptationsExplicitlyVerified': True, 'sourcePatchReplay': 'passed',
            'completeRustInventoryMatches': True}, 'Async source reconstruction incomplete')
    vendor = ROOT / 'vendor/polkadot-sdk/frame/staking-async'
    require(not any(path.is_symlink() for path in vendor.rglob('*')), 'Async fork contains a symlink outside its verified inventory')
    bound(V / 'verify-staking-async-source.py', source['verifierSha256'])
    for name, field in [('stable2609-customizations.json', 'provenanceSha256'),
            ('stable2609-customizations.patch', 'sourcePatchSha256'), ('Cargo.toml.orig', 'upstreamManifestSha256'),
            ('Cargo.toml', 'materializedManifestSha256'), ('.rustfmt.toml', 'rustfmtConfigSha256')]:
        bound(vendor / name, source[field])
    actual_rust = {str(p.relative_to(vendor)): sha(p) for p in vendor.rglob('*.rs')}
    require(actual_rust == source['rustFiles'], 'Async fork source inventory changed')
    require(len(actual_rust) == source['localRustFileCount'] > 0, 'Missing async fork source files')
    inventory_bytes = ''.join(f'{name}\0{digest}\n' for name, digest in sorted(actual_rust.items())).encode()
    require(hashlib.sha256(inventory_bytes).hexdigest() == source['localRustInventorySha256'], 'Async inventory aggregate differs')
    inventory = read('staking-public-inventory.json', True)
    imported = read('staking-cache-import.json')
    state_path = V / 'staking-public-state.json.gz'
    bound(state_path, inventory['stateSha256'])
    bound(state_path, imported['sourceSha256'])
    state = json.loads(gzip.decompress(state_path.read_bytes()))
    require(inventory['complete'] and state['complete'] and state['status'] == 'captured-complete-pinned-staking', 'Incomplete pinned staking inventory')
    require(state['blockHash'] == inventory['blockHash'] == imported['blockHash'] == snapshot['blockHash'], 'Staking pin differs')
    require(imported['status'] == 'imported-complete-pinned-public-state' and imported['existingValuesMatched'] is True, 'Staking cache import not verified')
    require(len(state['records']) == state['keys'] == inventory['totalKeys'] == imported['keysImported'], 'Pinned staking key counts differ')
    records = dict(state['records'])
    require(len(records) == state['keys'] and all(key.startswith(state['prefix']) for key in records), 'Invalid pinned staking key coverage')
    compatibility = read('wasm-compatibility.json', True)
    query = compatibility['stakingQueryCompatibility']
    bound(V / 'verify-staking-query-compatibility.cjs', query['inputs']['verifierSha256'])
    require(query['status'] == 'passed' and query['localOnly'] and query['remoteRequests'] == 0 and query['blockHash'] == snapshot['blockHash'], 'Incomplete offline staking query proof')
    require(query['inputs']['baselineMetadataSha256'] == compatibility['inputs']['baseline']['metadataScaleSha256'] and
            query['inputs']['candidateMetadataSha256'] == compatibility['inputs']['candidate']['metadataScaleSha256'], 'Client proof uses different metadata')
    bound(V / 'staking-public-inventory.json', query['inputs']['inventorySha256'])
    bound(state_path, query['inputs']['stateSha256'])
    # Re-run this small pure client check against all pinned values. No RPC or
    # Wasm execution is involved, and no recorded evidence is rewritten.
    query_code = r'''
const path = require('node:path');
const {verifyStakingQueryCompatibility} = require(path.join(process.argv[1], 'verify-staking-query-compatibility.cjs'));
console.log(JSON.stringify(verifyStakingQueryCompatibility({baselineMetadataPath:process.argv[2],
  candidateMetadataPath:process.argv[3], inventoryPath:path.join(process.argv[1], 'staking-public-inventory.json'),
  statePath:path.join(process.argv[1], 'staking-public-state.json.gz'), blockHash:process.argv[4]})));
'''
    client = subprocess.run([run['stages'][0]['command'][0], '-e', query_code, str(V),
            compatibility['inputs']['baseline']['metadataScalePath'], compatibility['inputs']['candidate']['metadataScalePath'], snapshot['blockHash']],
            capture_output=True, text=True, check=False)
    require(client.returncode == 0, f'Fresh pinned client query verification failed: {client.stderr}')
    require(json.loads(client.stdout) == query, 'Current client decode/JSON proof differs from recorded proof')
    # Independently reconstruct the exact allowed write set from the pinned raw
    # bytes. This helper only hashes storage keys; it never executes Wasm/RPC.
    node = run['stages'][0]['command'][0]
    code = r'''
const fs = require('node:fs'), zlib = require('node:zlib'), path = require('node:path');
const {xxhashAsHex} = require(path.join(process.argv[1], 'node_modules/@polkadot/util-crypto'));
const records = new Map(JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(process.argv[1], 'staking-public-state.json.gz')))).records);
const prefix = name => xxhashAsHex('Staking',128)+xxhashAsHex(name,128).slice(2);
const u32 = n => {const b=Buffer.alloc(4);b.writeUInt32LE(n);return b;};
const map = (name,key) => prefix(name)+xxhashAsHex(key,64).slice(2)+key.toString('hex');
const decode = key => Buffer.from(records.get(key).slice(2),'hex').readUInt32LE();
const active=decode(prefix('ActiveEra')), planned=decode(prefix('CurrentEra'));
const writes=new Map([[prefix(':__STORAGE_VERSION__:'),'0x1200'],[prefix('CutoffEra'),'0x'+u32(planned).toString('hex')],
  [prefix('FirstExecutionEra'),'0x'+u32(active).toString('hex')],[prefix('WeightedPointsFormulaStartEra'),'0x'+u32(active+1).toString('hex')]]);
for(let era=active;era<=planned;era++) for(const key of records.keys()) if(key.startsWith(map('ErasStakersOverview',u32(era))))
  writes.set(map('LastValidatorEra',Buffer.from(key.slice(-64),'hex')),'0x'+u32(era).toString('hex'));
console.log(JSON.stringify({active,planned,writes:[...writes],pointsKey:map('ErasRewardPoints',u32(active)),
  bridgeKey:xxhashAsHex('StakingAsyncBridge',128)+xxhashAsHex('State',128).slice(2),
  plannedStart:planned>active?decode(map('ErasStartSessionIndex',u32(planned))):null}));
'''
    process = subprocess.run([node, '-e', code, str(V)], capture_output=True, text=True, check=False)
    require(process.returncode == 0, f'Staking witness key reconstruction failed: {process.stderr}')
    expected = json.loads(process.stdout)
    expected_writes = dict(expected['writes'])
    require(records[expected['writes'][0][0]] == '0x1000', 'Pinned staking source storage version is not16')
    preserved = sorted([key, value] for key, value in records.items() if key not in expected_writes and key != expected['pointsKey'])
    preserved_hash = hashlib.sha256(json.dumps(preserved, separators=(',', ':')).encode()).hexdigest()
    proofs = {'inventorySha256': sha(V / 'staking-public-inventory.json'), 'stateSha256': sha(state_path),
              'importProofSha256': sha(V / 'staking-cache-import.json')}
    checks = policy['asyncStakingMigration']['checks']
    require(policy['checks']['fullPinnedStakingMigrationVerified'] is True and checks, 'Exact-Wasm migration witness missing')
    require([check['phase'] for check in checks] == ['initialize', 'finalize'], 'Staking witness must cover both real upgrade hooks and finalization')
    require(policy['upgradeTimestamp']['stakingWrites'] == 0, 'Timestamp inherent wrote unchecked staking state')
    for check in checks:
        require(check['passed'] and check['capturedKeys'] == len(records), 'Pinned staking witness incomplete')
        require(check['inputProofs'] == proofs, 'Pinned staking witness sources differ')
        require(check['reviewedMigrationWrites'] == expected['writes'] and check['normalAuthorshipPointsKey'] == expected['pointsKey'], 'Staking allowed changes differ from pinned state')
        require(check['bytePreservedKeys'] == len(preserved) and check['preservedBytesSha256'] == preserved_hash, 'Old staking bytes witness differs')
        require((check['storageVersion'], check['legacyCutoffEra'], check['firstExecutionEra'], check['weightedPointsCutoffEra'], check['recencyEntries']) ==
                (18, expected['planned'], expected['active'], expected['active'] + 1, len(expected_writes) - 4), 'Staking migration markers differ')
        for field in ('exactMigrationWritesVerified', 'oldLedgerClaimsExposuresBudgetsAndSlashBytesPreserved', 'noConsensusStateRootClaim'):
            require(check[field] is True, f'Missing staking witness assertion: {field}')
        points = check['authoredPoints']
        require(points['increment'] == 20 and points['allOtherPointsPreserved'] is True, 'Authorship point delta differs')
        require(re.fullmatch(r'0x[0-9a-fA-F]{64}', points['consensusAuthor']) and re.fullmatch(r'0x[0-9a-fA-F]{64}', points['rewardStash']), 'Missing author/stash identity')
        queued = None if expected['plannedStart'] is None else [expected['plannedStart'], expected['planned']]
        require(check['bridge'] == {'pending': None, 'queuedEra': queued, 'electionDeadline': None, 'forced': False, 'timestampEra': None}, 'Migration bridge state differs')
        queued_bytes = b'\0' if queued is None else b'\1' + b''.join(value.to_bytes(4, 'little') for value in queued)
        bridge_raw = '0x' + (b'\0' + queued_bytes + b'\0\0\0').hex()
        require(check['bridgeStorage'] == {'key': expected['bridgeKey'], 'value': bridge_raw}, 'Bridge seed must exist with exact SCALE bytes')
    summary = run['asyncStakingMigration']
    require(summary == {'reviewedCompatibility': True, 'storageVersions': {'from': 16, 'to': 18},
            'sourceProofSha256': sha(V / 'staking-async-source-proof.json'), 'completePinnedWitnessChecks': len(checks),
            'capturedKeys': len(records), 'fullPinnedStakingMigrationVerified': True}, 'Run migration summary differs')
    return {**summary, 'inputProofs': proofs, 'bytePreservedKeys': len(preserved),
            'preservedBytesSha256': preserved_hash, 'recencyEntries': len(expected_writes) - 4,
            'normalAuthorshipPointIncrement': 20, 'noConsensusStateRootClaim': True,
            'clientQueryCompatibility': query}

def verify_rejected(run, suites):
    reject = read('rejected-admission-check.json', True)
    review = read('rejected-admission-block-size-review.json', True)
    require(reject['candidateSha256'] == CANDIDATE and reject['networkRequests'] == 0, 'Rejected-admission scope differs')
    for key, name in [('snapshotSha256', 'snapshot.json'), ('runSha256', 'rehearsal-run.json'), ('compatibilitySha256', 'wasm-compatibility.json'), ('metadataSha256', 'candidate-metadata.json'), ('scriptSha256', 'verify-rejected-admission.cjs')]:
        bound(V / name, reject[key])
    all_true(reject['checks'], 'Rejected admission')
    hash_map(review['inputs'])
    require(review['checks']['negativeAndRollbackTestsPassed'] == 11, 'Missing rejection negative tests')
    all_true(review['checks'], 'BlockSize review', exceptions=('negativeAndRollbackTestsPassed',))
    require(review['sourceProof']['runtimeSourcesModified'] is False, 'BlockSize correction changed runtime source')
    require(review['upstreamRevisions']['after'] == UPSTREAM, 'BlockSize source comparison uses wrong SDK')
    for row in review['sourceProof']['checkWeightFunctions'].values():
        require(row['identicalBetween2603And2609'] is True, 'Block accounting function changed')
    require([s['name'] for s in review['steps']] == ['rejected-admission-regressions', 'rejected-admission-check'], 'Missing BlockSize validation steps')
    for s in review['steps']:
        require(s['exitCode'] == 0, 'Rejected-admission step failed')
        bound(V / s['log'], s['logSha256'])
    regression_log = (V / review['steps'][0]['log']).read_text()
    require(re.search(r'(?:#|ℹ) tests 11\b', regression_log) and re.search(r'(?:#|ℹ) pass 11\b', regression_log) and re.search(r'(?:#|ℹ) fail 0\b', regression_log), 'Rejection log does not contain 11 passing tests')
    for item in reject['suites']:
        name = item['report']; bound(V / name, item['reportSha256'])
        rows = [x for x in suites[name]['runtimeExtrinsicApplications'] if x['transaction'] == 'rolled_back_invalid_transaction']
        require(len(rows) == item['rejectedTransactions'], f'Rejected count differs: {name}')
        count = 0
        for row in rows:
            raw = json.dumps(row['discardedStorageDiff'], separators=(',', ':'), ensure_ascii=False).encode()
            require(hashlib.sha256(raw).hexdigest() == row['rawStorageDiffSha256'], 'Rejected raw diff hash mismatch')
            require(row['rawStorageDiffEntries'] == len(row['discardedStorageDiff']), 'Raw diff count mismatch')
            count += len(row['discardedStorageDiff'])
        require(count == item['inspectedRawDiffEntries'], 'Inspected raw diff total differs')
        require(item['rawAccountFeeRewardClaimAndEventWrites'] == item['committedStateChangesOnInvalidTransaction'] == 0, 'Rejected call changed protected state')
    require(sum(x['rejectedTransactions'] for x in reject['suites']) == 28, 'Expected 28 rejected fixture transactions')
    # Re-run the small pure verifier against the recorded raw diffs without
    # invoking its writing CLI or changing any evidence bound by the review.
    node = run['stages'][0]['command'][0]
    code = r"""
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const dir = process.argv[1];
const {accountingKeys, assertAccountingOnly} = require(path.join(dir, 'verify-rejected-admission.cjs'));
const read = name => JSON.parse(fs.readFileSync(path.join(dir, name), 'utf8'));
const allowed = accountingKeys(read('candidate-metadata.json'));
assert.deepEqual(allowed, read('rejected-admission-check.json').allowedAccountingKeys);
let transactions = 0, entries = 0;
for (const name of ['equivocation-fee-wasm-rehearsal.json', 'fee-policy-wasm-rehearsal.json']) {
  for (const row of read(name).runtimeExtrinsicApplications) {
    if (row.transaction !== 'rolled_back_invalid_transaction') continue;
    assertAccountingOnly(row.discardedStorageDiff, allowed);
    transactions++; entries += row.discardedStorageDiff.length;
  }
}
console.log(JSON.stringify({status: 'passed', transactions, entries}));
"""
    check = subprocess.run([node, '-e', code, str(V)], cwd=ROOT, capture_output=True, text=True, check=False)
    require(check.returncode == 0, f'Current pure rejected-diff verifier failed: {check.stderr}')
    pure = json.loads(check.stdout)
    require(pure == {'status': 'passed', 'transactions': 28, 'entries': sum(x['inspectedRawDiffEntries'] for x in reject['suites'])}, 'Current rejected-diff verifier totals differ')
    return {'reportSha256': sha(V / 'rejected-admission-check.json'), 'blockSizeReviewSha256': sha(V / 'rejected-admission-block-size-review.json'),
            'rejectedTransactions': sum(x['rejectedTransactions'] for x in reject['suites']),
            'inspectedRawDiffEntries': sum(x['inspectedRawDiffEntries'] for x in reject['suites']),
            'committedStateChangesOnInvalidTransaction': 0, 'negativeAndRollbackTests': 11,
            'currentPureVerifier': pure,
            'accountingException': 'Only four reviewed System block-accounting fields; BlockSize requires exact plain Optional u32 metadata and four-byte values. Every invalid diff is discarded.'}

def verify_bridge(snapshot):
    proof = read('bridge-readiness-verification.json', True)
    require(proof['activationAuthorized'] is False and len(proof['cases']) == 2, 'Bridge proof scope differs')
    bound(V / 'verify-bridge-readiness.cjs', proof['verifierSha256'])
    bound(V / 'bridge-readiness-policy.cjs', proof['policySha256'])
    bound(V / 'bridge-rpc-size.cjs', proof['sizeNormalizerSha256'])
    bound(V / 'bridge-cancellation-review.json', proof['cancellationReviewSha256'])
    node = read('rehearsal-run.json', True)['stages'][0]['command'][0]
    recovery_proof = read('bridge-payload-recovery-verification.json', True)
    for key, name in {'payloadEvidenceSha256': 'bridge-payload-evidence.json',
            'sourceRequestsSha256': 'bridge-request-recovery-source.json',
            'inventorySha256': 'bridge-request-recovery-capture.json',
            'archivedSourceSha256': 'bridge-archived-payload-source.json',
            'baselineMetadataSha256': 'baseline-metadata.scale',
            'candidateMetadataSha256': 'candidate-metadata.scale',
            'verifierSha256': 'verify-bridge-payload-recovery.cjs'}.items():
        bound(V / name, recovery_proof['inputs'][key])
    capture_helper = V / 'verify-bridge-capture.py'
    capture_tests = V / 'test-bridge-capture.py'
    sha(capture_helper); sha(capture_tests)
    spec = importlib.util.spec_from_file_location('bridge_capture_lineage', capture_helper)
    capture_module = importlib.util.module_from_spec(spec); spec.loader.exec_module(capture_module)
    capture_lineage = capture_module.verify_capture(V / 'bridge-request-recovery-capture.json',
        V / 'bridge-request-recovery-source.json', V / 'bridge-request-recovery-pages.jsonl.gz')
    tests = subprocess.run([sys.executable, str(capture_tests)], cwd=ROOT, capture_output=True, text=True, check=False)
    require(tests.returncode == 0 and re.search(r'Ran 17 tests', tests.stderr) and 'OK' in tests.stderr,
            f'Bridge capture negative regressions failed: {tests.stdout} {tests.stderr}')
    capture_lineage['testEvidence'] = {'command': [sys.executable, str(capture_tests)], 'passed': 17,
        'testScriptSha256': sha(capture_tests), 'stdout': tests.stdout, 'stderr': tests.stderr}
    code = "const p=require('path'),v=p.resolve(process.argv[1]); require(p.join(v,'verify-bridge-payload-recovery.cjs')).verifyRecovery({evidencePath:p.join(v,'bridge-payload-evidence.json'),requestsPath:p.join(v,'bridge-request-recovery-source.json'),archivePath:p.join(v,'bridge-archived-payload-source.json'),inventoryPath:p.join(v,'bridge-request-recovery-capture.json'),baselineMetadataPath:p.join(v,'baseline-metadata.scale'),candidateMetadataPath:p.join(v,'candidate-metadata.scale')}).then(r=>console.log(JSON.stringify(r))).catch(e=>{console.error(e);process.exitCode=1;});"
    result = subprocess.run([node, '-e', code, str(V)], cwd=ROOT, capture_output=True, text=True, check=False)
    require(result.returncode == 0 and json.loads(result.stdout) == recovery_proof, f'Bridge payload reconstruction differs: {result.stderr}')
    for case, report_name, snapshot_name in zip(proof['cases'],
            ('bridge-readiness.json', 'bridge-activation-current.json'),
            ('snapshot.json', 'bridge-activation-snapshot.json')):
        require(case['report'] == report_name and case['snapshot'] == snapshot_name, 'Bridge evidence set/order differs')
        require(case['payloadBoundsResolved'] and case['payloadBoundsVerified']
                and case['globalStoredCallInventoryVerified'] and case['globalStoredCallBoundsVerified'],
                'Bridge payload/global stored-call bound gate failed')
        require(case['newCandidatePayloadRegressions'] == case['newlyDecodableLegacyPayloads'] == 0,
                'Candidate changes execution admission for a captured pending payload')
        bound(V / report_name, case['reportSha256'])
        bound(V / snapshot_name, case['snapshotSha256'])
        b = read(report_name, True)
        require(b['candidateBinding']['sha256'] == CANDIDATE, 'Bridge candidate differs')
        require(b['submittedTransactions'] == b['privateKeysRead'] == 0, 'Bridge query not read-only')
        if b.get('payloadEvidenceBinding'):
            bound(path(b['payloadEvidenceBinding']['path']), b['payloadEvidenceBinding']['sha256'])
        if b.get('operatorEvidenceBinding'):
            bound(path(b['operatorEvidenceBinding']['path']), b['operatorEvidenceBinding']['sha256'])
        command = [node, '-e', "const v=require(process.argv[1]); console.log(JSON.stringify(v.verify(process.argv[2],process.argv[3])));",
                   str(V / 'verify-bridge-readiness.cjs'), report_name, snapshot_name]
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        require(result.returncode == 0, f'Independent bridge evidence check failed: {result.stderr}')
        require(json.loads(result.stdout) == case, 'Recorded bridge policy result differs from raw-evidence replay')
    recovery = read('bridge-cancellation-review.json')
    require(recovery['status'] == 'unsigned-review-only' and recovery['submittedTransactions'] == 0,
            'Recovery calls must remain unsigned and unsubmitted')
    command = [node, '-e', "const v=require(process.argv[1]); console.log(JSON.stringify(v.recoveryManifest('bridge-activation-current.json')));", str(V / 'verify-bridge-readiness.cjs')]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
    require(result.returncode == 0 and json.loads(result.stdout) == recovery, 'Recovery calls differ from candidate metadata and raw pending inventory')
    return {'reportSha256': sha(V / 'bridge-readiness-verification.json'),
            'pinned': {k: v for k, v in proof['cases'][0].items() if k not in ('operations', 'accounts')},
            'current': {k: v for k, v in proof['cases'][1].items() if k not in ('operations', 'accounts')},
            'unsignedRecoveryCalls': len(recovery['operations']), 'compatibleOperationsPreserved': len(recovery['preservedOperations']),
            'recoveredPayloads': recovery_proof['payloadCount'], 'payloadClassifications': recovery_proof['classifications'],
            'rawCaptureLineage': capture_lineage, 'activationAuthorized': False}

def verify_native(fingerprint, snapshot):
    native = read('native-regressions.json', True)
    remote = read('native-remote-execution.json', True)
    bound(V / 'source-clippy-start.json', native['sourceFingerprintSha256'])
    require([s['name'] for s in native['steps']] == EXPECTED_NATIVE, 'Native regression step set/order differs')
    totals = {'passed': 0, 'failed': 0, 'ignored': 0}
    groups = []
    no_rust_tests = {'source-start', 'source-end', 'native-pinned-production'}
    for s in native['steps']:
        require(s['exitCode'] == 0 and s.get('finishedAt'), f'Native step unfinished: {s["name"]}')
        bound(V / s['log'], s['logSha256'])
        counts = rust_counts((V / s['log']).read_text(errors='replace'))
        require(counts == s['testResults'], f'Native recorded counts differ from log: {s["name"]}')
        tally = {k: sum(x[k] for x in counts) for k in totals}
        require(tally['failed'] == 0, 'Native tests failed')
        if s['name'] not in no_rust_tests:
            require(tally['passed'] > 0, f'Native step silently ran no tests: {s["name"]}')
        groups.append({'name': s['name'], **tally})
        for key in totals: totals[key] += tally[key]
    for name in ['source-regressions-start-check.json', 'source-regressions-end-check.json']:
        source_check(name, fingerprint)
    bound(V / 'native-remote-scope.json', remote['scopeSha256'])
    bound(V / 'snapshot.json', remote['snapshotSha256'])
    require(Path(remote['sourceFingerprint']).resolve() == (V / 'source-clippy-start.json').resolve(), 'Native remote fingerprint differs')
    hash_map(remote['sourceInputs'])
    require(remote['sourceCommit'] == fingerprint['git']['head'], 'Native source commit differs')
    require(remote['submittedTransactions'] == remote['privateKeysRead'] == 0, 'Native scope differs')
    require([s['stage'] for s in remote['sourceChecks']] == ['start', 'end'], 'Native source checks incomplete')
    for row in remote['sourceChecks']:
        require(row['exitCode'] == 0, 'Native source identity check failed')
        bound(V / row['evidence'], row['evidenceSha256'])
        source_check(row['evidence'], fingerprint)
    graph = remote['featureGraph']
    require(graph['exitCode'] == 0 and graph['status'] == 'passed', 'Native feature graph failed')
    bound(V / graph['evidence'], graph['sha256'])
    bound(V / graph['stderrEvidence'], graph['stderrSha256'])
    forbidden = {'private-net', 'stage', 'wip', 'runtime-benchmarks', 'reduced-pswap-reward-periods'}
    require({'std', 'try-runtime'} <= set(graph['runtimeFeatures']), 'Native production features missing')
    for package in graph['packages']:
        require(not forbidden.intersection(package['features']), f'Nonproduction native dependency feature: {package}')
    require(len(remote['results']) == 2, 'Native remote test phases missing')
    for index, result in enumerate(remote['results']):
        require(result['exitCode'] == 0, 'Native remote command failed')
        bound(V / result['log'], result['logSha256'])
        text = (V / result['log']).read_text(errors='replace')
        expected = 4 if index == 0 else 1
        require(rust_counts(text) == [{'passed': expected, 'failed': 0, 'ignored': 0}], 'Unexpected remote test counts')
        require('Skipping remote migration test' not in text, 'Native remote replay skipped')
        if index == 1:
            require(f"block={snapshot['blockNumber']}, hash={snapshot['blockHash']}" in text and 'Runtime upgrade rehearsal replay:' in text, 'Pinned replay not in native log')
    state = remote['stateSnapshot']
    bound(V / state['path'], state['sha256'])
    require((V / state['path']).stat().st_size == state['bytes'] > 0, 'Native snapshot size mismatch')
    scope = read('native-remote-scope.json')
    require(scope['status'] == 'preflight-passed-scope-prepared', 'Scope preflight did not pass')
    require(scope['sourceBlockHash'] == snapshot['blockHash'] and scope['sourceBlockNumber'] == snapshot['blockNumber'], 'Native scope has different pinned block')
    bound(V / 'snapshot.json', scope['inputs']['snapshotSha256'])
    bound(V / 'deployed-metadata.scale', scope['inputs']['metadataSha256'])
    bound(V / 'live-runtime.compact.compressed.wasm', scope['inputs']['deployedCodeSha256'])
    bound(V / 'prepare-native-remote.cjs', scope['inputs']['scriptSha256'])
    bound(ROOT / 'runtime-upgrade-4.8.11/validation/native-remote-scope.json', scope['inputs']['priorScopeTemplateSha256'])
    bound(RUN / 'rpc-provider-rdap.json', scope['inputs']['providerRdapSha256'])
    require(scope['preflight']['readOnly'] and scope['preflight']['submittedTransactions'] == scope['preflight']['privateKeysRead'] == 0, 'Scope preflight not read-only')
    require(remote['environment']['REMOTE_UPGRADE_CHECKS'] == 'pre-and-post' and remote['environment']['REQUIRE_REMOTE'] == '1', 'Native upgrade checks weakened')
    require(remote['environment']['REMOTE_STATE_VERSION'] == str(snapshot['runtimeVersion']['stateVersion']), 'Native trie version mismatch')
    expected_guards = {'DemeterFarmingPlatform': ('0x02', 'V3'), 'CeresGovernancePlatform': ('0x02', 'V3'), 'CeresTokenLocker': ('0x01', 'V2'), 'CeresLiquidityLocker': ('0x01', 'V2'), 'HermesGovernancePlatform': ('0x01', 'V2')}
    require({x['pallet']: (x['raw'], x['decodedVersion']) for x in scope['preflight']['customVersionGuards']} == expected_guards, 'Legacy custom version guards differ')
    return {'reportSha256': sha(V / 'native-regressions.json'), 'remoteReportSha256': sha(V / 'native-remote-execution.json'),
            'regressionSteps': len(native['steps']), 'testExecutions': totals,
            'remoteModeTestsPassed': 4, 'pinnedMigrationTestsPassed': 1,
            'totalRustTestExecutionsPassed': totals['passed'] + 5,
            'countsMeaning': 'Test executions; targeted reruns can include cases also covered by a complete package suite.',
            'groups': groups, 'stateSnapshot': state, 'scopeSha256': remote['scopeSha256'],
            'nativeTestInstrumentation': graph['nativeTestInstrumentation'],
            'coverage': {**remote['coverage'], 'preparedScopeStatus': remote['coverage']['status'], 'status': remote['status']},
            'limitations': remote['limitations']}

def fresh_source_check(fingerprint):
    command = [sys.executable, str(V / 'source-fingerprint.py'), '--check', str(V / 'source-clippy-start.json'), '--source-only']
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
    require(result.returncode == 0, f'Current complete source inventory differs: {result.stderr}\n{result.stdout}')
    d = json.loads(result.stdout)
    require(d['status'] == 'passed' and d['sourceSha256'] == SOURCE and d['changes'] == [], 'Fresh source check failed')
    return {'command': command, 'result': d, 'stderr': result.stderr}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--precheck', action='store_true', help='Read-only check of completed non-native gates; never writes final evidence')
    args = parser.parse_args()
    if not args.precheck:
        # Fail before any other work if either required native gate is unfinished.
        read('native-regressions.json', True)
        read('native-remote-execution.json', True)
    build, proof, fingerprint = verify_build()
    run, snapshot, metadata, suites = verify_wasm()
    audits = verify_audits(run)
    staking = verify_staking_migration(run, snapshot, suites['fee-policy-wasm-rehearsal.json'])
    rejected = verify_rejected(run, suites)
    bridge = verify_bridge(snapshot)
    if args.precheck:
        print(json.dumps({'status': 'completed-non-native-gates-pass', 'finalSummaryWritten': False, 'verifiedFileBindings': len(verified)}))
        return
    native = verify_native(fingerprint, snapshot)
    fresh = fresh_source_check(fingerprint)
    top_files = ['final-build-verification.json', 'candidate-build-evidence.json', 'source-clippy-start.json',
        'native-regressions.json', 'native-remote-execution.json', 'native-remote-scope.json', 'rehearsal-run.json',
        'current-production-identity.json', 'snapshot.json', 'wasm-compatibility.json', 'metadata-compatibility.json',
        'python-candidate-signing-report.json', 'equivocation-fee-wasm-rehearsal.json', 'fee-policy-wasm-rehearsal.json',
        'fee-event-check.json', 'bridge-readiness.json', 'bridge-activation-current.json', 'bridge-activation-snapshot.json', 'bridge-readiness-verification.json', 'bridge-cancellation-review.json', 'bridge-payload-recovery-verification.json', 'bridge-payload-evidence.json', 'bridge-request-recovery-source.json', 'bridge-archived-payload-source.json', 'bridge-request-recovery-capture.json', 'bridge-request-recovery-pages.jsonl.gz', 'verify-bridge-capture.py', 'test-bridge-capture.py', 'rejected-admission-check.json', 'rejected-admission-block-size-review.json',
        'sdk-primitives-provenance.json', 'staking-async-source-proof.json', 'staking-public-inventory.json', 'staking-public-state.json.gz', 'staking-cache-import.json', 'current-cargo-audits/summary.json', 'current-cargo-audits/database-source.json', 'imported-migration-evidence/import-summary.json', 'imported-migration-evidence/historical-reference.json', 'npm-security-refresh/summary.json', 'npm-security-refresh/installed-checks.json']
    summary = {'status': 'passed', 'checkedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'project': 'SORA Nexus', 'sdk': {'tag': 'polkadot-stable2609', 'commit': UPSTREAM},
        'candidate': {'path': proof['candidatePath'], 'sha256': CANDIDATE, 'bytes': proof['candidateBytes'], 'specVersion': 137, 'transactionVersion': 133, 'systemVersion': 0},
        'sourceFingerprint': proof['sourceFingerprint'], 'freshSourceVerification': fresh,
        'clippy': {'status': 'passed', 'modes': ['mainnet', 'try-runtime', 'extended'], 'allTargets': True, 'denyWarnings': True, 'compilerWarnings': 0, 'compilerErrors': 0},
        'native': native, 'wasm': {'runId': run['runId'], 'stagesPassed': len(run['stages']), 'checks': run['checks'], 'sdkMigration': run['sdkMigration'], 'metadataCounts': metadata['counts'], 'rawScaleCompatible': False, 'reviewedSdkMigrationCompatible': True},
        'productionBaseline': {'endpoint': snapshot['endpoint'], 'blockNumber': snapshot['blockNumber'], 'blockHash': snapshot['blockHash'], 'specVersion': snapshot['runtimeVersion']['specVersion'], 'transactionVersion': snapshot['runtimeVersion']['transactionVersion'], 'codeSha256': snapshot['code']['sha256']},
        'audits': audits, 'asyncStakingMigration': staking, 'rejectedAdmission': rejected, 'bridge': bridge,
        'submittedTransactions': 0, 'deployed': False,
        'scopeLimitations': [
            'Native storage is scoped; it is not whole-chain try_state or a complete state-root proof.',
            'Native tests enable common/test instrumentation; exact default-production Wasm is verified separately.',
            'Synthetic fixtures and mocked outer signatures exercise local paths; heartbeat signatures and migration ownership proofs use real Wasm verification.',
            'Production reward-publication/remap and BABE-repair markers are already set; live replay checks idempotence, and local fixtures cover active historical paths.',
            'Bridge evidence results separately record global stored-call bounds, unresolved payloads, and trusted operator readiness; unsigned cancellation plans do not establish completed recovery or activation authority.',
            'System.CodeUpdated gains a hash field, Offences field naming changes, and new authorization/Identity metadata requires client refresh and transaction version 133.',
            'Actual async staking replaces synchronous staking; the migration preserves pinned legacy bytes and local session/election behavior through a runtime adapter. Full Staking coverage is not a whole-chain state-root proof.',
            'Unchanged SDK client/security harness executions are explicitly historical; current runtime and async pallet suites, dependency audits, and Clippy are fresh.',
            'Zero native submissions is based on reviewed read-only RPC code paths, not independent packet capture.',
        ],
        'evidenceSha256': {name: sha(V / name) for name in top_files},
        'aggregatorSha256': sha(Path(__file__)), 'verifiedFileBindings': len(verified)}
    # Ensure nothing previously inspected changed during aggregation.
    for name in list(verified): sha(Path(name))
    output = V / 'rehearsal-summary.json'
    temporary = output.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(summary, indent=2) + '\n')
    os.replace(temporary, output)
    print(json.dumps({'status': 'passed', 'summary': str(output), 'sha256': sha(output), 'rustTestExecutionsPassed': native['totalRustTestExecutionsPassed']}))

if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, AssertionError) as error:
        print(f'final verification refused: {error}', file=sys.stderr)
        sys.exit(1)
